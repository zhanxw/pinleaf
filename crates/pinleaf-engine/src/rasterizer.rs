use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use hayro_interpret::font::GlyphRun;
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, RenderContent, SoftMask, TransformExt, interpret_page,
};
use kurbo::{BezPath, Rect};
use pinleaf_core::{NormRect, PageIndex};

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

#[derive(Default)]
struct ImageRectDevice {
    rects: Vec<NormRect>,
}

impl<'a> Device<'a> for ImageRectDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
    fn draw_glyph_run(&mut self, _: &GlyphRun<'_, 'a>, _: DrawProps<'a>, _: &DrawMode) {}

    fn draw_image(&mut self, image: Image<'a, '_>, props: ImageDrawProps<'a>) {
        if !matches!(image, Image::Raster(_)) {
            return;
        }
        let local = Rect::new(0.0, 0.0, image.width() as f64, image.height() as f64);
        let bbox = props.transform.transform_rect_bbox(local);
        let rect = NormRect::new(
            bbox.x0 as f32,
            bbox.y0 as f32,
            bbox.x1 as f32,
            bbox.y1 as f32,
        );
        if rect.width() >= 16.0 && rect.height() >= 16.0 {
            self.rects.push(rect);
        }
    }
}

/// Extracts Normalized Page Space bounding rectangles for raster figures/images on a page so
/// Smart Dark Mode can keep diagrams/photos un-inverted.
pub fn extract_page_image_rects(handle: &PdfDocumentHandle, page_idx: PageIndex) -> Vec<NormRect> {
    let Ok(meta) = handle.page_meta(page_idx) else {
        return Vec::new();
    };
    let Some(page) = handle.pdf().pages().get(page_idx.as_usize()) else {
        return Vec::new();
    };

    let initial_transform = page.initial_transform(true).to_kurbo();
    let target = Rect::new(0.0, 0.0, meta.norm_width as f64, meta.norm_height as f64);
    let cache = InterpreterCache::new();
    let settings = InterpreterSettings {
        render_content: RenderContent::PAGE,
        ..Default::default()
    };

    let mut ctx = Context::new(initial_transform, target, &cache, page.xref(), settings);
    let mut device = ImageRectDevice::default();
    interpret_page(page, &mut ctx, &mut device);
    device.rects
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    use std::sync::Arc;

    #[test]
    fn extract_page_image_rects_finds_raster_figure_bounds() {
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let img_pixels: Vec<u8> = vec![
            255, 0, 0, 0, 255, 0, // row 1: red, green
            0, 0, 255, 255, 255, 0, // row 2: blue, yellow
        ];
        let img_id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 2,
                "Height" => 2,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            img_pixels,
        ));

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Im0" => img_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        120.into(),
                        0.into(),
                        0.into(),
                        90.into(),
                        50.into(),
                        500.into(),
                    ],
                ),
                Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
                Operation::new("Q", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();

        let handle = PdfDocumentHandle::from_bytes(Arc::new(bytes)).unwrap();
        let rects = extract_page_image_rects(&handle, PageIndex(0));
        assert_eq!(rects.len(), 1);
        let r = rects[0];
        assert!((r.min_x - 50.0).abs() < 1.0);
        assert!((r.max_x - 170.0).abs() < 1.0);
        assert!((r.min_y - (792.0 - 590.0)).abs() < 1.0);
        assert!((r.max_y - (792.0 - 500.0)).abs() < 1.0);
    }
}
