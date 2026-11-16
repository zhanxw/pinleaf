use compact_str::CompactString;
use hayro_interpret::font::{Glyph, GlyphRun};
use hayro_interpret::hayro_cmap::{BfString, WritingMode};
use hayro_interpret::{BlendMode, ClipPath, Device, DrawMode, DrawProps, Image, ImageDrawProps, SoftMask};
use kurbo::{Affine, BezPath, Point};
use pinleaf_core::{NormPoint, NormQuad, NormRect};

#[derive(Clone, Debug)]
pub struct ExtractedGlyph {
    pub text: CompactString,
    pub rect: NormRect,
    pub quad: NormQuad,
    pub baseline_y: f32,
    pub font_size: f32,
}

/// Custom `hayro_interpret::Device` that captures positioned Unicode glyphs in Normalized Page Space
/// while skipping rasterization of paths, clips, and images.
pub struct TextExtractionDevice {
    pub glyphs: Vec<ExtractedGlyph>,
    last_stroke_dedup_sig: Option<(usize, u32, u32)>,
}

impl TextExtractionDevice {
    pub fn new() -> Self {
        Self {
            glyphs: Vec::new(),
            last_stroke_dedup_sig: None,
        }
    }
}

impl Default for TextExtractionDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Device<'a> for TextExtractionDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}

    fn push_clip_path(&mut self, _: &ClipPath) {}

    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}

    fn pop_clip(&mut self) {}

    fn pop_transparency_group(&mut self) {}

    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}

    fn draw_glyph_run(
        &mut self,
        glyph_run: &GlyphRun<'_, 'a>,
        props: DrawProps<'a>,
        draw_mode: &DrawMode,
    ) {
        let glyphs = glyph_run.glyphs();
        if glyphs.is_empty() {
            return;
        }

        // In PDF TextRenderingMode::FillStroke (Tr 2), hayro emits Fill first and Stroke second
        // for the exact same run. Deduplicate immediate Stroke duplicates without dropping Stroke-only (Tr 1).
        let first_m = props.transform * glyphs[0].transform();
        let first_origin = first_m * Point::ZERO;
        let run_sig = (
            glyphs.len(),
            (first_origin.x * 10.0).round() as i32 as u32,
            (first_origin.y * 10.0).round() as i32 as u32,
        );
        if matches!(draw_mode, DrawMode::Stroke(_))
            && self.last_stroke_dedup_sig == Some(run_sig)
        {
            return;
        }
        if matches!(draw_mode, DrawMode::Fill(_)) {
            self.last_stroke_dedup_sig = Some(run_sig);
        } else {
            self.last_stroke_dedup_sig = None;
        }

        let horizontal_writing = glyph_run.writing_mode() == WritingMode::Horizontal;

        for pos_glyph in glyphs {
            let unicode = match pos_glyph.as_unicode() {
                Some(BfString::Char(c)) => {
                    if c.is_control() && c != '\t' {
                        continue;
                    }
                    let mut s = CompactString::default();
                    s.push(c);
                    s
                }
                Some(BfString::String(s)) => {
                    let filtered: CompactString =
                        s.chars().filter(|&c| !c.is_control() || c == '\t').collect();
                    if filtered.is_empty() {
                        continue;
                    }
                    filtered
                }
                None => continue,
            };

            let m: Affine = props.transform * pos_glyph.transform();
            let adv_upem = match &**pos_glyph {
                Glyph::Outline(g) => g.advance_width().unwrap_or(500.0),
                Glyph::Type3(g) => g.advance_width().unwrap_or(500.0),
            }
            .max(120.0) as f64;

            // Measure vertical UPEM unit vector to determine visual font size in points.
            let p_base = m * Point::ZERO;
            let p_top_ref = m * Point::new(0.0, 1000.0);
            let font_size = ((p_top_ref.x - p_base.x).hypot(p_top_ref.y - p_base.y) as f32).max(2.0);

            let (x0, x1, y_bot, y_top) = if horizontal_writing {
                (0.0, adv_upem, -220.0, 820.0)
            } else {
                (-450.0, 450.0, -900.0, 100.0)
            };

            // Transform the 4 corners from UPEM glyph space directly into Normalized Page Space.
            let c_tl = m * Point::new(x0, y_top);
            let c_tr = m * Point::new(x1, y_top);
            let c_bl = m * Point::new(x0, y_bot);
            let c_br = m * Point::new(x1, y_bot);

            let pts = [
                NormPoint::new(c_tl.x as f32, c_tl.y as f32),
                NormPoint::new(c_tr.x as f32, c_tr.y as f32),
                NormPoint::new(c_bl.x as f32, c_bl.y as f32),
                NormPoint::new(c_br.x as f32, c_br.y as f32),
            ];
            let quad = NormQuad::from_unordered_points(pts);
            let rect = quad.bounding_rect();

            let baseline_mid = m * Point::new(adv_upem * 0.5, 0.0);

            self.glyphs.push(ExtractedGlyph {
                text: unicode,
                rect,
                quad,
                baseline_y: baseline_mid.y as f32,
                font_size,
            });
        }
    }
}
