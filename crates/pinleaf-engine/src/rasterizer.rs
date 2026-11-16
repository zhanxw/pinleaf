use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use hayro_interpret::{InterpreterSettings, RenderContent};
use pinleaf_core::PageIndex;

use crate::document::{EngineError, PdfDocumentHandle};

const MAX_RENDER_DIM_PX: f32 = 4096.0;

#[derive(Clone, Debug)]
pub struct BgraFrame {
    pub width: u32,
    pub height: u32,
    pub pixels_bgra: Vec<u8>,
}

/// Renders a single PDF page off-thread into premultiplied BGRA8 pixels for GPUI `RenderImage`.
///
/// Annotations are excluded from the rasterized page (`RenderContent::PAGE`) so GPUI can render
/// and interactively update highlights, underlines, and notes across all panes without re-rendering.
pub fn render_page_bgra(
    handle: &PdfDocumentHandle,
    page_idx: PageIndex,
    requested_scale: f32,
) -> Result<BgraFrame, EngineError> {
    let meta = handle.page_meta(page_idx)?;
    let page = handle
        .pdf()
        .pages()
        .get(page_idx.as_usize())
        .ok_or_else(|| EngineError::PageOutOfBounds(page_idx.0, handle.page_count()))?;

    let scale = clamp_scale(meta.norm_width, meta.norm_height, requested_scale);

    let interpreter_settings = InterpreterSettings {
        render_content: RenderContent::PAGE,
        ..Default::default()
    };
    let render_settings = RenderSettings::default();
    let pixmap_settings = PixmapSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: WHITE,
    };

    let cache = RenderCache::new();
    let pixmap = render(
        page,
        &cache,
        &interpreter_settings,
        &render_settings,
        &pixmap_settings,
    );

    let width = pixmap.width() as u32;
    let height = pixmap.height() as u32;
    if width == 0 || height == 0 {
        return Err(EngineError::RenderFailed(
            page_idx.0,
            "rendered zero-sized pixmap".to_string(),
        ));
    }

    let mut pixels_bgra = pixmap.data_as_u8_slice().to_vec();
    for px in pixels_bgra.chunks_exact_mut(4) {
        px.swap(0, 2); // RGBA -> BGRA for Metal / GPUI RenderImage
    }

    Ok(BgraFrame {
        width,
        height,
        pixels_bgra,
    })
}

fn clamp_scale(norm_width: f32, norm_height: f32, requested_scale: f32) -> f32 {
    let min_scale = (1.0 / norm_width.min(norm_height).max(1.0)).max(0.05);
    let max_scale_w = MAX_RENDER_DIM_PX / norm_width.max(1.0);
    let max_scale_h = MAX_RENDER_DIM_PX / norm_height.max(1.0);
    let max_scale = max_scale_w.min(max_scale_h).max(min_scale);
    requested_scale.clamp(min_scale, max_scale)
}
