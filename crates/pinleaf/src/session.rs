use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use gpui::{AppContext, Context, RenderImage};
use image::{Frame, RgbaImage};
use lru::LruCache;
use pinleaf_annot::{load_annotations, save_annotations_atomic, save_annotations_to_bytes};
use pinleaf_core::{
    Annotation, AnnotationId, AnnotationStore, NormRect, PageIndex, PageTransform, PortalLink,
    ReadingAnchor, RgbaColor, annotation::pdf_now_string, find_active_portal,
};
use pinleaf_engine::demo_fixture::build_demo_pdf_bytes;
use pinleaf_engine::{
    LinkRegion, PageMeta, PageTextLayout, PdfDocumentHandle, SearchMatch, SyncTexMap,
    SyncTexPdfLoc, TocEntry, extract_all_links, extract_page_image_rects, extract_page_text,
    extract_toc, render_page_bgra, search_layouts,
};

pub const DEFAULT_CUSTOM_BG_RGB: [u8; 3] = [0x1e, 0x1e, 0x2e];
pub const DEFAULT_CUSTOM_FG_RGB: [u8; 3] = [0xcd, 0xd6, 0xf4];

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ColorMode {
    #[default]
    Light,
    Dark,
    Sepia,
    Custom,
}

impl ColorMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Sepia,
            Self::Sepia => Self::Custom,
            Self::Custom => Self::Light,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Sepia => "Sepia",
            Self::Custom => "Custom",
        }
    }
}

/// Applies reading color mode transform in-place on premultiplied BGRA8 pixels.
#[cfg(test)]
pub fn apply_color_mode_bgra(pixels_bgra: &mut [u8], mode: ColorMode) {
    let width = (pixels_bgra.len() / 4).max(1) as u32;
    apply_color_mode_bgra_with_rects(
        pixels_bgra,
        width,
        1,
        mode,
        (DEFAULT_CUSTOM_BG_RGB, DEFAULT_CUSTOM_FG_RGB),
        &[],
    );
}

/// Applies reading color mode transform in-place on premultiplied BGRA8 pixels while preserving
/// raster figure rectangles (`[x0, y0, x1, y1]`) un-inverted in `Dark` and `Custom` modes.
pub fn apply_color_mode_bgra_with_rects(
    pixels_bgra: &mut [u8],
    width: u32,
    height: u32,
    mode: ColorMode,
    custom_bg_fg: ([u8; 3], [u8; 3]),
    preserve_rects_px: &[[u32; 4]],
) {
    if mode == ColorMode::Light || width == 0 || height == 0 {
        return;
    }

    let in_preserved = |x: u32, y: u32| -> bool {
        preserve_rects_px
            .iter()
            .any(|&[x0, y0, x1, y1]| x >= x0 && x < x1 && y >= y0 && y < y1)
    };

    let (bg_rgb, fg_rgb) = custom_bg_fg;
    let w_usize = width as usize;

    for (idx, px) in pixels_bgra.chunks_exact_mut(4).enumerate() {
        let x = (idx % w_usize) as u32;
        let y = (idx / w_usize) as u32;

        if !preserve_rects_px.is_empty()
            && matches!(mode, ColorMode::Dark | ColorMode::Custom)
            && in_preserved(x, y)
        {
            continue;
        }

        match mode {
            ColorMode::Light => {}
            ColorMode::Sepia => {
                px[0] = (px[0] as f32 * 0.82).clamp(16.0, 240.0) as u8;
                px[1] = (px[1] as f32 * 0.92).clamp(14.0, 246.0) as u8;
                px[2] = (px[2] as f32 * 0.97).clamp(18.0, 250.0) as u8;
            }
            ColorMode::Dark => {
                let b = px[0] as f32;
                let g = px[1] as f32;
                let r = px[2] as f32;
                let max_c = r.max(g).max(b);
                let min_c = r.min(g).min(b);
                if (max_c - min_c) < 18.0 {
                    px[0] = (28.0 + (255.0 - b) * (200.0 / 255.0)).clamp(0.0, 255.0) as u8;
                    px[1] = (22.0 + (255.0 - g) * (210.0 / 255.0)).clamp(0.0, 255.0) as u8;
                    px[2] = (20.0 + (255.0 - r) * (215.0 / 255.0)).clamp(0.0, 255.0) as u8;
                } else {
                    let mid = 0.5 * (max_c + min_c);
                    let target_mid = 232.0 - mid * (200.0 / 255.0);
                    let delta = target_mid - mid;
                    px[0] = (b + delta).clamp(24.0, 250.0) as u8;
                    px[1] = (g + delta).clamp(20.0, 250.0) as u8;
                    px[2] = (r + delta).clamp(18.0, 250.0) as u8;
                }
            }
            ColorMode::Custom => {
                let b = px[0] as f32;
                let g = px[1] as f32;
                let r = px[2] as f32;
                let lum_t = ((0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0).clamp(0.0, 1.0);
                let out_r = fg_rgb[0] as f32 + (bg_rgb[0] as f32 - fg_rgb[0] as f32) * lum_t;
                let out_g = fg_rgb[1] as f32 + (bg_rgb[1] as f32 - fg_rgb[1] as f32) * lum_t;
                let out_b = fg_rgb[2] as f32 + (bg_rgb[2] as f32 - fg_rgb[2] as f32) * lum_t;
                px[0] = out_b.clamp(0.0, 255.0) as u8;
                px[1] = out_g.clamp(0.0, 255.0) as u8;
                px[2] = out_r.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileCacheKey {
    pub page: PageIndex,
    pub zoom_bucket: i16,
    pub color_mode: ColorMode,
    pub preserve_figures: bool,
    pub custom_bg_fg: ([u8; 3], [u8; 3]),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThumbCacheKey {
    pub page: PageIndex,
    pub color_mode: ColorMode,
    pub preserve_figures: bool,
    pub custom_bg_fg: ([u8; 3], [u8; 3]),
}

/// Target thumbnail rasterization scale capped for crisp ~240 px max dimension thumbnails.
pub fn thumbnail_scale_for_page(norm_w: f32, norm_h: f32) -> f32 {
    (240.0 / norm_w.max(norm_h).max(1.0)).clamp(0.12, 0.5)
}

fn render_styled_page_bgra(
    handle: &PdfDocumentHandle,
    page: PageIndex,
    scale: f32,
    norm_w: f32,
    norm_h: f32,
    mode: ColorMode,
    preserve_figures: bool,
    custom_bg_fg: ([u8; 3], [u8; 3]),
    cached_rects: Option<Arc<Vec<NormRect>>>,
) -> std::result::Result<(u32, u32, Vec<u8>, Option<Arc<Vec<NormRect>>>), pinleaf_engine::EngineError> {
    let mut frame = render_page_bgra(handle, page, scale)?;
    let computed_rects = if preserve_figures {
        Some(cached_rects.unwrap_or_else(|| Arc::new(extract_page_image_rects(handle, page))))
    } else {
        None
    };

    let px_rects: Vec<[u32; 4]> = computed_rects
        .as_deref()
        .map(|rects| {
            let sx = frame.width as f32 / norm_w.max(1.0);
            let sy = frame.height as f32 / norm_h.max(1.0);
            rects
                .iter()
                .map(|r| {
                    [
                        (r.min_x * sx).floor().clamp(0.0, frame.width as f32) as u32,
                        (r.min_y * sy).floor().clamp(0.0, frame.height as f32) as u32,
                        (r.max_x * sx).ceil().clamp(0.0, frame.width as f32) as u32,
                        (r.max_y * sy).ceil().clamp(0.0, frame.height as f32) as u32,
                    ]
                })
                .collect()
        })
        .unwrap_or_default();

    apply_color_mode_bgra_with_rects(
        &mut frame.pixels_bgra,
        frame.width,
        frame.height,
        mode,
        custom_bg_fg,
        &px_rects,
    );
    Ok((frame.width, frame.height, frame.pixels_bgra, computed_rects))
}

/// Quantizes continuous zoom scale into ~19% exponential buckets (`2^(bucket / 4)`) so
/// scrolling/splitting reuses cached page textures efficiently without blurriness.
pub fn scale_to_zoom_bucket(scale: f32) -> i16 {
    let clamped = scale.clamp(0.25, 4.5);
    (clamped.log2() * 4.0).round() as i16
}

pub fn zoom_bucket_to_scale(bucket: i16) -> f32 {
    2.0_f32.powf(bucket as f32 / 4.0).clamp(0.25, 4.5)
}

#[derive(Clone, Debug, Default)]
pub struct SearchState {
    pub query: String,
    pub regex_mode: bool,
    pub matches: Vec<SearchMatch>,
    pub active_idx: usize,
}

/// Shared document model observed by every open split pane and OS window displaying this PDF.
pub struct DocumentSession {
    pub path: Option<PathBuf>,
    pub title: String,
    pub pdf_bytes: Arc<Vec<u8>>,
    pub pdf_handle: Arc<PdfDocumentHandle>,
    pub page_metas: Vec<PageMeta>,
    pub page_transforms: Vec<PageTransform>,
    pub links_by_page: HashMap<PageIndex, Vec<LinkRegion>>,
    pub toc_entries: Vec<TocEntry>,
    bitmap_cache: LruCache<TileCacheKey, Arc<RenderImage>>,
    in_flight_renders: HashSet<TileCacheKey>,
    thumb_cache: LruCache<ThumbCacheKey, Arc<RenderImage>>,
    in_flight_thumbs: HashSet<ThumbCacheKey>,
    text_cache: LruCache<PageIndex, Arc<PageTextLayout>>,
    content_bounds_cache: HashMap<PageIndex, (f32, f32)>,
    image_rects_cache: HashMap<PageIndex, Arc<Vec<NormRect>>>,
    pub all_layouts: Option<Arc<Vec<Arc<PageTextLayout>>>>,
    indexing_all_layouts: bool,
    pub color_mode: ColorMode,
    pub smart_dark_mode: bool,
    pub custom_bg_rgb: [u8; 3],
    pub custom_fg_rgb: [u8; 3],
    pub auto_reload: bool,
    last_mtime: Option<SystemTime>,
    watcher_started: bool,
    pub marks: HashMap<char, ReadingAnchor>,
    pub portals: Vec<PortalLink>,
    pub next_portal_id: u64,
    pub pending_portal_src: Option<(PageIndex, f32)>,
    pub synctex: Option<Arc<SyncTexMap>>,
    pub synctex_highlight: Option<(PageIndex, NormRect)>,
    pub search: SearchState,
    pub annotations: AnnotationStore,
    pub status_message: Option<(String, bool)>,
}

impl DocumentSession {
    pub fn open_path(path: PathBuf) -> Result<Self> {
        let bytes = std::fs::read(&path)?;
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Document.pdf")
            .to_string();
        Self::from_bytes(Some(path), title, bytes)
    }

    pub fn open_demo() -> Result<Self> {
        let bytes = build_demo_pdf_bytes();
        Self::from_bytes(
            None,
            "Pinleaf Demo — Freeze & Branch Reading.pdf".to_string(),
            bytes,
        )
    }

    pub fn from_bytes(path: Option<PathBuf>, title: String, bytes: Vec<u8>) -> Result<Self> {
        let pdf_bytes = Arc::new(bytes);
        let pdf_handle = Arc::new(PdfDocumentHandle::from_bytes(pdf_bytes.clone())?);
        let page_metas = pdf_handle.page_metas().to_vec();
        let page_transforms = pdf_handle.page_transforms();
        let links_by_page = extract_all_links(&pdf_bytes, &page_transforms);
        let toc_entries = extract_toc(&pdf_bytes, &pdf_handle);
        let loaded_annots = load_annotations(&pdf_bytes, &page_transforms).unwrap_or_default();
        let annotations = AnnotationStore::from_loaded(loaded_annots);
        let last_mtime = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        let synctex = path
            .as_ref()
            .and_then(|p| SyncTexMap::load_for_pdf(p))
            .map(Arc::new);

        Ok(Self {
            path,
            title,
            pdf_bytes,
            pdf_handle,
            page_metas,
            page_transforms,
            links_by_page,
            toc_entries,
            bitmap_cache: LruCache::new(NonZeroUsize::new(72).unwrap()),
            in_flight_renders: HashSet::new(),
            thumb_cache: LruCache::new(NonZeroUsize::new(256).unwrap()),
            in_flight_thumbs: HashSet::new(),
            text_cache: LruCache::new(NonZeroUsize::new(64).unwrap()),
            content_bounds_cache: HashMap::new(),
            image_rects_cache: HashMap::new(),
            all_layouts: None,
            indexing_all_layouts: false,
            color_mode: ColorMode::Light,
            smart_dark_mode: true,
            custom_bg_rgb: DEFAULT_CUSTOM_BG_RGB,
            custom_fg_rgb: DEFAULT_CUSTOM_FG_RGB,
            auto_reload: true,
            last_mtime,
            watcher_started: false,
            marks: HashMap::new(),
            portals: Vec::new(),
            next_portal_id: 1,
            pending_portal_src: None,
            synctex,
            synctex_highlight: None,
            search: SearchState::default(),
            annotations,
            status_message: None,
        })
    }

    pub fn page_count(&self) -> usize {
        self.page_metas.len()
    }

    pub fn best_available_bitmap(
        &mut self,
        page: PageIndex,
        target_bucket: i16,
    ) -> (Option<Arc<RenderImage>>, bool) {
        let mode = self.color_mode;
        let preserve_figures = self.smart_dark_mode && matches!(mode, ColorMode::Dark | ColorMode::Custom);
        let custom_bg_fg = (self.custom_bg_rgb, self.custom_fg_rgb);
        let exact_key = TileCacheKey {
            page,
            zoom_bucket: target_bucket,
            color_mode: mode,
            preserve_figures,
            custom_bg_fg,
        };
        if let Some(img) = self.bitmap_cache.get(&exact_key) {
            return (Some(img.clone()), true);
        }

        let mut best: Option<(i16, Arc<RenderImage>)> = None;
        for (k, img) in self.bitmap_cache.iter() {
            if k.page == page
                && k.color_mode == mode
                && k.preserve_figures == preserve_figures
                && k.custom_bg_fg == custom_bg_fg
            {
                let dist = (k.zoom_bucket - target_bucket).abs();
                if best.as_ref().is_none_or(|(bd, _)| dist < *bd) {
                    best = Some((dist, img.clone()));
                }
            }
        }
        (best.map(|(_, img)| img), false)
    }

    pub fn request_page_bitmap(
        &mut self,
        page: PageIndex,
        zoom_bucket: i16,
        cx: &mut Context<Self>,
    ) {
        let mode = self.color_mode;
        let preserve_figures = self.smart_dark_mode && matches!(mode, ColorMode::Dark | ColorMode::Custom);
        let custom_bg_fg = (self.custom_bg_rgb, self.custom_fg_rgb);
        let key = TileCacheKey {
            page,
            zoom_bucket,
            color_mode: mode,
            preserve_figures,
            custom_bg_fg,
        };
        if self.bitmap_cache.contains(&key) || self.in_flight_renders.contains(&key) {
            return;
        }
        self.in_flight_renders.insert(key);
        let handle = self.pdf_handle.clone();
        let scale = zoom_bucket_to_scale(zoom_bucket);
        let (norm_w, norm_h) = self
            .page_metas
            .get(page.as_usize())
            .map(|m| (m.norm_width, m.norm_height))
            .unwrap_or((612.0, 792.0));
        let cached_image_rects = self.image_rects_cache.get(&page).cloned();

        cx.spawn(async move |session, cx| {
            let rendered = cx
                .background_spawn(async move {
                    render_styled_page_bgra(
                        &handle,
                        page,
                        scale,
                        norm_w,
                        norm_h,
                        mode,
                        preserve_figures,
                        custom_bg_fg,
                        cached_image_rects,
                    )
                })
                .await;

            let _ = session.update(cx, |session, cx| {
                session.in_flight_renders.remove(&key);
                if let Ok((width, height, pixels_bgra, computed_rects)) = rendered {
                    if let Some(rects) = computed_rects {
                        session.image_rects_cache.insert(page, rects);
                    }
                    if let Some(rgba_buf) = RgbaImage::from_raw(width, height, pixels_bgra) {
                        let img = Arc::new(RenderImage::new(vec![Frame::new(rgba_buf)]));
                        if let Some((old_key, evicted)) = session.bitmap_cache.push(key, img)
                            && old_key != key
                        {
                            cx.drop_image(evicted, None);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn get_or_request_thumbnail(
        &mut self,
        page: PageIndex,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let mode = self.color_mode;
        let preserve_figures =
            self.smart_dark_mode && matches!(mode, ColorMode::Dark | ColorMode::Custom);
        let custom_bg_fg = (self.custom_bg_rgb, self.custom_fg_rgb);
        let key = ThumbCacheKey {
            page,
            color_mode: mode,
            preserve_figures,
            custom_bg_fg,
        };
        if let Some(img) = self.thumb_cache.get(&key) {
            return Some(img.clone());
        }
        if self.in_flight_thumbs.contains(&key) || self.in_flight_thumbs.len() >= 8 {
            return None;
        }
        self.in_flight_thumbs.insert(key);
        let handle = self.pdf_handle.clone();
        let (norm_w, norm_h) = self
            .page_metas
            .get(page.as_usize())
            .map(|m| (m.norm_width, m.norm_height))
            .unwrap_or((612.0, 792.0));
        let scale = thumbnail_scale_for_page(norm_w, norm_h);
        let cached_image_rects = self.image_rects_cache.get(&page).cloned();

        cx.spawn(async move |session, cx| {
            let rendered = cx
                .background_spawn(async move {
                    render_styled_page_bgra(
                        &handle,
                        page,
                        scale,
                        norm_w,
                        norm_h,
                        mode,
                        preserve_figures,
                        custom_bg_fg,
                        cached_image_rects,
                    )
                })
                .await;

            let _ = session.update(cx, |session, cx| {
                session.in_flight_thumbs.remove(&key);
                if let Ok((width, height, pixels_bgra, computed_rects)) = rendered {
                    if let Some(rects) = computed_rects {
                        session.image_rects_cache.insert(page, rects);
                    }
                    if let Some(rgba_buf) = RgbaImage::from_raw(width, height, pixels_bgra) {
                        let img = Arc::new(RenderImage::new(vec![Frame::new(rgba_buf)]));
                        if let Some((old_key, evicted)) = session.thumb_cache.push(key, img)
                            && old_key != key
                        {
                            cx.drop_image(evicted, None);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        None
    }

    pub fn ensure_text_layout(&mut self, page: PageIndex) -> Arc<PageTextLayout> {
        if let Some(ref all) = self.all_layouts
            && let Some(found) = all.get(page.as_usize())
        {
            return found.clone();
        }
        if let Some(existing) = self.text_cache.get(&page) {
            return existing.clone();
        }
        let layout = extract_page_text(&self.pdf_handle, page).unwrap_or(PageTextLayout {
            page,
            full_text: String::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
        });
        let arc = Arc::new(layout);
        self.text_cache.put(page, arc.clone());
        arc
    }

    pub fn trigger_background_text_index(&mut self, cx: &mut Context<Self>) {
        if self.all_layouts.is_some() || self.indexing_all_layouts {
            return;
        }
        self.indexing_all_layouts = true;
        let handle = self.pdf_handle.clone();
        let page_count = self.page_count();

        cx.spawn(async move |session, cx| {
            let layouts = cx
                .background_spawn(async move {
                    let mut all = Vec::with_capacity(page_count);
                    for i in 0..page_count {
                        let page = PageIndex(i as u32);
                        let layout = extract_page_text(&handle, page).unwrap_or(PageTextLayout {
                            page,
                            full_text: String::new(),
                            glyphs: Vec::new(),
                            lines: Vec::new(),
                        });
                        all.push(Arc::new(layout));
                    }
                    Arc::new(all)
                })
                .await;

            let _ = session.update(cx, |session, cx| {
                // Only apply if pdf_handle hasn't changed while indexing.
                session.indexing_all_layouts = false;
                for l in layouts.iter().take(16) {
                    session.text_cache.put(l.page, l.clone());
                }
                session.all_layouts = Some(layouts);
                if !session.search.query.is_empty() {
                    session.run_search_with_cached_layouts();
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn ensure_all_layouts_now(&mut self) -> Arc<Vec<Arc<PageTextLayout>>> {
        if let Some(ref all) = self.all_layouts {
            return all.clone();
        }
        let count = self.page_count();
        if count <= 40 {
            let mut all = Vec::with_capacity(count);
            for i in 0..count {
                all.push(self.ensure_text_layout(PageIndex(i as u32)));
            }
            let arc = Arc::new(all);
            self.all_layouts = Some(arc.clone());
            return arc;
        }
        let mut partial = Vec::with_capacity(count.min(20));
        for i in 0..count.min(20) {
            partial.push(self.ensure_text_layout(PageIndex(i as u32)));
        }
        Arc::new(partial)
    }

    fn run_search_with_cached_layouts(&mut self) {
        if self.search.query.is_empty() {
            self.search.matches.clear();
            self.search.active_idx = 0;
            return;
        }
        let layouts = self.ensure_all_layouts_now();
        self.search.matches = search_layouts(&layouts, &self.search.query, self.search.regex_mode);
        if self.search.matches.is_empty() {
            self.search.active_idx = 0;
        } else {
            self.search.active_idx = self.search.active_idx.min(self.search.matches.len() - 1);
        }
    }

    pub fn update_search(&mut self, query: String, regex_mode: bool, cx: &mut Context<Self>) {
        self.search.query = query;
        self.search.regex_mode = regex_mode;
        self.search.active_idx = 0;
        if self.search.query.is_empty() {
            self.search.matches.clear();
            cx.notify();
            return;
        }
        self.run_search_with_cached_layouts();
        if self.all_layouts.is_none() {
            self.trigger_background_text_index(cx);
        }
        cx.notify();
    }

    pub fn step_search_match(
        &mut self,
        delta: i32,
        cx: &mut Context<Self>,
    ) -> Option<(PageIndex, f32)> {
        let len = self.search.matches.len();
        if len == 0 {
            return None;
        }
        let next = ((self.search.active_idx as i32 + delta).rem_euclid(len as i32)) as usize;
        self.search.active_idx = next;
        let m = &self.search.matches[next];
        let target = (m.page, m.top_norm_y);
        cx.notify();
        Some(target)
    }

    pub fn current_search_match(&self) -> Option<(PageIndex, f32)> {
        self.search
            .matches
            .get(self.search.active_idx)
            .map(|m| (m.page, m.top_norm_y))
    }

    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search = SearchState::default();
        cx.notify();
    }

    pub fn cycle_color_mode(&mut self, cx: &mut Context<Self>) {
        self.color_mode = self.color_mode.cycle();
        self.status_message = Some((format!("Color mode: {}", self.color_mode.label()), false));
        cx.notify();
    }

    pub fn toggle_smart_dark_mode(&mut self, cx: &mut Context<Self>) {
        self.smart_dark_mode = !self.smart_dark_mode;
        self.status_message = Some((
            format!(
                "Smart Figure Preservation: {}",
                if self.smart_dark_mode { "ON" } else { "OFF" }
            ),
            false,
        ));
        cx.notify();
    }

    pub fn add_portal(
        &mut self,
        src_page: PageIndex,
        src_focal_y: f32,
        dst: ReadingAnchor,
        label: String,
        cx: &mut Context<Self>,
    ) -> PortalLink {
        let id = self.next_portal_id;
        self.next_portal_id += 1;
        let portal = PortalLink {
            id,
            src_page,
            src_focal_y,
            dst,
            label,
        };
        self.portals.push(portal.clone());
        self.pending_portal_src = None;
        self.status_message = Some((
            format!(
                "Portal #{id} linked: p.{} -> p.{}",
                src_page.one_based(),
                dst.page.one_based()
            ),
            false,
        ));
        cx.notify();
        portal
    }

    pub fn delete_nearest_portal(
        &mut self,
        page: PageIndex,
        focal_y: f32,
        cx: &mut Context<Self>,
    ) -> Option<PortalLink> {
        let active_id = find_active_portal(&self.portals, page, focal_y)?.id;
        let pos = self.portals.iter().position(|p| p.id == active_id)?;
        let removed = self.portals.remove(pos);
        self.status_message = Some((format!("Removed Portal #{}", removed.id), false));
        cx.notify();
        Some(removed)
    }

    pub fn apply_synctex_forward(
        &mut self,
        tex_path: &Path,
        line: u32,
        cx: &mut Context<Self>,
    ) -> Option<SyncTexPdfLoc> {
        if self.synctex.is_none()
            && let Some(ref p) = self.path
        {
            self.synctex = SyncTexMap::load_for_pdf(p).map(Arc::new);
        }
        let loc = self.synctex.as_ref()?.forward_search(tex_path, line)?;
        self.synctex_highlight = Some((loc.page, loc.rect));
        self.status_message = Some((
            format!(
                "SyncTeX forward -> p.{} (line {line})",
                loc.page.one_based()
            ),
            false,
        ));
        cx.notify();
        Some(loc)
    }

    pub fn content_x_bounds_read(&self, page: PageIndex) -> (f32, f32) {
        if let Some(&cached) = self.content_bounds_cache.get(&page) {
            return cached;
        }
        let pw = self
            .page_metas
            .get(page.as_usize())
            .map(|m| m.norm_width)
            .unwrap_or(612.0);
        let layout_opt = self
            .all_layouts
            .as_ref()
            .and_then(|all| all.get(page.as_usize()).cloned())
            .or_else(|| self.text_cache.peek(&page).cloned());
        if let Some(layout) = layout_opt {
            compute_content_x_from_layout(&layout, pw)
        } else {
            (pw * 0.08, pw * 0.92)
        }
    }

    pub fn content_x_bounds(&mut self, page: PageIndex) -> (f32, f32) {
        if let Some(&cached) = self.content_bounds_cache.get(&page) {
            return cached;
        }
        let pw = self
            .page_metas
            .get(page.as_usize())
            .map(|m| m.norm_width)
            .unwrap_or(612.0);
        let layout = self.ensure_text_layout(page);
        let bounds = compute_content_x_from_layout(&layout, pw);
        self.content_bounds_cache.insert(page, bounds);
        bounds
    }

    pub fn links_for_page(&self, page: PageIndex) -> &[LinkRegion] {
        self.links_by_page
            .get(&page)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn add_annotation(&mut self, ann: Annotation, cx: &mut Context<Self>) {
        self.annotations.insert(ann);
        self.status_message = None;
        cx.notify();
    }

    pub fn update_annotation_color(
        &mut self,
        id: AnnotationId,
        color: RgbaColor,
        cx: &mut Context<Self>,
    ) {
        if self
            .annotations
            .update_color(id, color, pdf_now_string())
        {
            self.status_message = None;
            cx.notify();
        }
    }

    pub fn update_annotation_category(
        &mut self,
        id: AnnotationId,
        category: Option<char>,
        color: RgbaColor,
        cx: &mut Context<Self>,
    ) {
        if self
            .annotations
            .update_category_and_color(id, category, color, pdf_now_string())
        {
            self.status_message = None;
            cx.notify();
        }
    }

    pub fn update_annotation_note(
        &mut self,
        id: AnnotationId,
        note: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.annotations.update_note(id, note, pdf_now_string()) {
            self.status_message = None;
            cx.notify();
        }
    }

    pub fn delete_annotation(&mut self, id: AnnotationId, cx: &mut Context<Self>) {
        if self.annotations.delete(id) {
            self.status_message = None;
            cx.notify();
        }
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if self.annotations.undo() {
            self.status_message = None;
            cx.notify();
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if self.annotations.redo() {
            self.status_message = None;
            cx.notify();
        }
    }

    pub fn save_to_current_path(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        self.save_to_path(&path, cx);
        true
    }

    /// Returns a self-contained PDF byte buffer suitable for printing, baking in any
    /// unsaved annotation edits in memory without modifying the file on disk.
    pub fn export_printable_pdf_bytes(&self) -> Vec<u8> {
        let diff = self.annotations.pending_diff();
        if diff.is_empty() {
            return (*self.pdf_bytes).clone();
        }
        match save_annotations_to_bytes(&self.pdf_bytes, &diff, &self.page_transforms) {
            Ok(saved) => saved.bytes,
            Err(_) => (*self.pdf_bytes).clone(),
        }
    }

    pub fn save_to_path(&mut self, target_path: &Path, cx: &mut Context<Self>) {
        let diff = self.annotations.pending_diff();
        let result = if diff.is_empty() && self.path.as_deref() == Some(target_path) {
            self.status_message = Some(("No unsaved changes".to_string(), false));
            cx.notify();
            return;
        } else if diff.is_empty() {
            std::fs::write(target_path, &*self.pdf_bytes).map(|_| pinleaf_annot::SaveResult {
                bytes: (*self.pdf_bytes).clone(),
                assigned_ids: Vec::new(),
            })
            .map_err(anyhow::Error::from)
        } else {
            save_annotations_atomic(&self.pdf_bytes, target_path, &diff, &self.page_transforms)
                .map_err(anyhow::Error::from)
        };

        match result {
            Ok(saved) => {
                if let Ok(new_handle) = PdfDocumentHandle::from_bytes(Arc::new(saved.bytes.clone())) {
                    self.pdf_bytes = new_handle.raw_bytes().clone();
                    self.pdf_handle = Arc::new(new_handle);
                }
                self.annotations.mark_saved(&saved.assigned_ids);
                self.path = Some(target_path.to_path_buf());
                self.last_mtime = std::fs::metadata(target_path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                self.start_file_watcher(cx);
                self.title = target_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Document.pdf")
                    .to_string();
                self.status_message = Some((format!("Saved {}", self.title), false));
            }
            Err(err) => {
                // Also test non-disk save helper only if target failed; show error banner.
                let _ = save_annotations_to_bytes(&self.pdf_bytes, &diff, &self.page_transforms);
                self.status_message = Some((format!("Save failed: {err}"), true));
            }
        }
        cx.notify();
    }

    pub fn toggle_auto_reload(&mut self, cx: &mut Context<Self>) {
        self.auto_reload = !self.auto_reload;
        if self.auto_reload {
            self.start_file_watcher(cx);
        }
        self.status_message = Some((
            format!("Auto-reload: {}", if self.auto_reload { "ON" } else { "OFF" }),
            false,
        ));
        cx.notify();
    }

    pub fn apply_reloaded_snapshot(&mut self, snap: ReloadedSnapshot, cur_mtime: Option<SystemTime>, cx: &mut Context<Self>) {
        self.last_mtime = cur_mtime;
        self.pdf_bytes = snap.pdf_bytes;
        self.pdf_handle = snap.pdf_handle;
        self.page_metas = snap.page_metas;
        self.page_transforms = snap.page_transforms;
        self.links_by_page = snap.links_by_page;
        self.toc_entries = snap.toc_entries;
        self.synctex = snap.synctex;

        while let Some((_, img)) = self.bitmap_cache.pop_lru() {
            cx.drop_image(img, None);
        }
        self.in_flight_renders.clear();
        while let Some((_, img)) = self.thumb_cache.pop_lru() {
            cx.drop_image(img, None);
        }
        self.in_flight_thumbs.clear();
        self.text_cache.clear();
        self.content_bounds_cache.clear();
        self.image_rects_cache.clear();
        self.all_layouts = None;
        self.indexing_all_layouts = false;

        if !self.annotations.is_dirty() {
            self.annotations = AnnotationStore::from_loaded(snap.loaded_annots);
        }
        if !self.search.query.is_empty() {
            self.run_search_with_cached_layouts();
        }
        self.trigger_background_text_index(cx);
        cx.notify();
    }

    pub fn force_reload_from_disk(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ref path) = self.path else {
            return false;
        };
        let Some((snap, loaded_mtime)) = parse_reloaded_snapshot(path) else {
            return false;
        };
        self.apply_reloaded_snapshot(snap, loaded_mtime, cx);
        self.status_message = Some(("Reloaded PDF from disk".to_string(), false));
        true
    }

    pub fn start_file_watcher(&mut self, cx: &mut Context<Self>) {
        if self.watcher_started || self.path.is_none() {
            return;
        }
        self.watcher_started = true;
        cx.spawn(async move |session, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(800))
                    .await;
                let Ok((path_opt, enabled, prev_mtime)) =
                    session.read_with(cx, |s, _| (s.path.clone(), s.auto_reload, s.last_mtime))
                else {
                    break;
                };
                if !enabled {
                    continue;
                }
                let Some(path) = path_opt else {
                    continue;
                };
                let cur_mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                let changed = match (cur_mtime, prev_mtime) {
                    (Some(cur), Some(prev)) => cur > prev,
                    (Some(_), None) => true,
                    _ => false,
                };
                if !changed {
                    continue;
                }

                // Wait briefly so pdflatex/typst finishes atomic write before reading.
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;

                let path_clone = path.clone();
                let snapshot_opt = cx
                    .background_spawn(async move { parse_reloaded_snapshot(&path_clone) })
                    .await;

                let Some((snap, loaded_mtime)) = snapshot_opt else {
                    continue;
                };

                let _ = session.update(cx, |s, cx| {
                    if s.path.as_deref() != Some(path.as_path()) {
                        return;
                    }
                    s.apply_reloaded_snapshot(snap, loaded_mtime.or(cur_mtime), cx);
                    s.status_message = Some(("Auto-reloaded updated PDF".to_string(), false));
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

pub struct ReloadedSnapshot {
    pdf_bytes: Arc<Vec<u8>>,
    pdf_handle: Arc<PdfDocumentHandle>,
    page_metas: Vec<PageMeta>,
    page_transforms: Vec<PageTransform>,
    links_by_page: HashMap<PageIndex, Vec<LinkRegion>>,
    toc_entries: Vec<TocEntry>,
    loaded_annots: Vec<Annotation>,
    synctex: Option<Arc<SyncTexMap>>,
}

fn parse_reloaded_snapshot(path: &Path) -> Option<(ReloadedSnapshot, Option<SystemTime>)> {
    let loaded_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    let bytes = std::fs::read(path).ok()?;
    let pdf_bytes = Arc::new(bytes);
    let pdf_handle = Arc::new(PdfDocumentHandle::from_bytes(pdf_bytes.clone()).ok()?);
    let page_metas = pdf_handle.page_metas().to_vec();
    let page_transforms = pdf_handle.page_transforms();
    let links_by_page = extract_all_links(&pdf_bytes, &page_transforms);
    let toc_entries = extract_toc(&pdf_bytes, &pdf_handle);
    let loaded_annots = load_annotations(&pdf_bytes, &page_transforms).unwrap_or_default();
    let synctex = SyncTexMap::load_for_pdf(path).map(Arc::new);
    Some((
        ReloadedSnapshot {
            pdf_bytes,
            pdf_handle,
            page_metas,
            page_transforms,
            links_by_page,
            toc_entries,
            loaded_annots,
            synctex,
        },
        loaded_mtime,
    ))
}

fn compute_content_x_from_layout(layout: &PageTextLayout, page_width: f32) -> (f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    for line in &layout.lines {
        if line.bounds.width() > 2.0 {
            min_x = min_x.min(line.bounds.min_x);
            max_x = max_x.max(line.bounds.max_x);
        }
    }
    if min_x.is_finite() && max_x.is_finite() && (max_x - min_x) >= page_width * 0.25 {
        (
            (min_x - 14.0).clamp(0.0, page_width),
            (max_x + 14.0).clamp(0.0, page_width),
        )
    } else {
        (page_width * 0.08, page_width * 0.92)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_mode_bgra_transforms_light_dark_and_sepia() {
        let orig = [255u8, 255, 255, 255, 0, 0, 0, 255, 240, 60, 20, 255];

        let mut light = orig;
        apply_color_mode_bgra(&mut light, ColorMode::Light);
        assert_eq!(light, orig);

        let mut sepia = orig;
        apply_color_mode_bgra(&mut sepia, ColorMode::Sepia);
        assert!(sepia[0] < sepia[1] && sepia[1] < sepia[2]);

        let mut dark = orig;
        apply_color_mode_bgra(&mut dark, ColorMode::Dark);
        // White paper becomes dark slate BGRA [28, 22, 20]
        assert_eq!(&dark[0..4], &[28, 22, 20, 255]);
        // Black ink becomes bright off-white [228, 232, 235]
        assert_eq!(&dark[4..8], &[228, 232, 235, 255]);
        // Blue accent stays blue-dominant
        assert!(dark[8] > dark[10]);

        // Smart figure preservation keeps pixels inside [1, 0, 2, 1] completely unchanged in Dark and Custom modes
        let mut preserved = [
            255u8, 255, 255, 255, // (0, 0) paper
            40, 200, 30, 255, // (1, 0) figure pixel -> preserved
            0, 0, 0, 255, // (2, 0) ink
        ];
        apply_color_mode_bgra_with_rects(
            &mut preserved,
            3,
            1,
            ColorMode::Custom,
            ([0x1e, 0x1e, 0x2e], [0xcd, 0xd6, 0xf4]),
            &[[1, 0, 2, 1]],
        );
        assert_eq!(&preserved[0..4], &[0x2e, 0x1e, 0x1e, 255]);
        assert_eq!(&preserved[4..8], &[40, 200, 30, 255]);
        assert_eq!(&preserved[8..12], &[0xf4, 0xd6, 0xcd, 255]);
    }

    #[test]
    fn thumbnail_scale_for_page_bounds_aspect_ratios() {
        let letter = thumbnail_scale_for_page(612.0, 792.0);
        assert!((letter - 240.0 / 792.0).abs() < 1e-4);

        let a4 = thumbnail_scale_for_page(595.0, 842.0);
        assert!((a4 - 240.0 / 842.0).abs() < 1e-4);

        let wide_slide = thumbnail_scale_for_page(1920.0, 1080.0);
        assert!((wide_slide - 0.125).abs() < 1e-4);

        let huge_poster = thumbnail_scale_for_page(5000.0, 4000.0);
        assert_eq!(huge_poster, 0.12);

        let tiny = thumbnail_scale_for_page(100.0, 100.0);
        assert_eq!(tiny, 0.5);
    }

    #[test]
    fn export_printable_pdf_bytes_includes_unsaved_annotations() {
        let mut session = DocumentSession::open_demo().expect("demo PDF loads");
        assert_eq!(session.annotations.all_sorted().len(), 1);
        let rect = pinleaf_core::NormRect::new(72.0, 140.0, 260.0, 156.0);
        session.annotations.insert(pinleaf_core::Annotation {
            id: pinleaf_core::AnnotationId::new(),
            pdf_object_id: None,
            page: PageIndex(0),
            kind: pinleaf_core::AnnotationKind::Highlight,
            quads: vec![pinleaf_core::NormQuad::from_rect(rect)],
            bounds: rect,
            color: pinleaf_core::RgbaColor::YELLOW,
            category: Some('t'),
            selected_text: Some("Theorem 1".to_string()),
            note_contents: None,
            modified_at: "D:20261117101500Z".to_string(),
        });
        assert!(session.annotations.is_dirty());

        let bytes = session.export_printable_pdf_bytes();
        let reloaded = pinleaf_annot::load_annotations(&bytes, &session.page_transforms)
            .expect("exported printable PDF annotations parse");
        assert_eq!(reloaded.len(), 2);
        assert!(session.annotations.is_dirty());
    }
}
