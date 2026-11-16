use std::ops::Range;

use hayro_interpret::{
    Context, InterpreterCache, InterpreterSettings, RenderContent, TransformExt, interpret_page,
};
use kurbo::Rect;
use pinleaf_core::{NormPoint, NormQuad, NormRect, PageIndex};

use crate::document::{EngineError, PdfDocumentHandle};
use crate::text_device::{ExtractedGlyph, TextExtractionDevice};

#[derive(Clone, Debug)]
pub struct GlyphSpan {
    pub byte_range: Range<usize>,
    pub rect: NormRect,
    pub line_index: u32,
    pub is_synthetic_space: bool,
}

#[derive(Clone, Debug)]
pub struct TextLine {
    pub line_index: u32,
    pub span_range: Range<usize>,
    pub bounds: NormRect,
    pub baseline_y: f32,
    pub font_size: f32,
}

#[derive(Clone, Debug)]
pub struct PageTextLayout {
    pub page: PageIndex,
    pub full_text: String,
    pub glyphs: Vec<GlyphSpan>,
    pub lines: Vec<TextLine>,
}

/// Extracts reading-order text layout with Normalized Page Space hit-testing and quad merging.
pub fn extract_page_text(
    handle: &PdfDocumentHandle,
    page_idx: PageIndex,
) -> Result<PageTextLayout, EngineError> {
    let meta = handle.page_meta(page_idx)?;
    let page = handle
        .pdf()
        .pages()
        .get(page_idx.as_usize())
        .ok_or_else(|| EngineError::PageOutOfBounds(page_idx.0, handle.page_count()))?;

    let initial_transform = page.initial_transform(true).to_kurbo();
    let target = Rect::new(0.0, 0.0, meta.norm_width as f64, meta.norm_height as f64);
    let cache = InterpreterCache::new();
    let settings = InterpreterSettings {
        render_content: RenderContent::PAGE,
        ..Default::default()
    };

    let mut ctx = Context::new(initial_transform, target, &cache, page.xref(), settings);
    let mut device = TextExtractionDevice::new();
    interpret_page(page, &mut ctx, &mut device);

    Ok(PageTextLayout::build(
        page_idx,
        device.glyphs,
        meta.norm_width,
    ))
}

#[derive(Clone)]
struct RawLine {
    glyphs: Vec<ExtractedGlyph>,
    bounds: NormRect,
    baseline_y: f32,
    font_size: f32,
}

impl PageTextLayout {
    pub fn build(page: PageIndex, mut raw_glyphs: Vec<ExtractedGlyph>, page_width: f32) -> Self {
        if raw_glyphs.is_empty() {
            return Self {
                page,
                full_text: String::new(),
                glyphs: Vec::new(),
                lines: Vec::new(),
            };
        }

        // Cluster glyphs into horizontal lines by baseline Y proximity.
        raw_glyphs.sort_by(|a, b| {
            a.baseline_y
                .total_cmp(&b.baseline_y)
                .then_with(|| a.rect.min_x.total_cmp(&b.rect.min_x))
        });

        let mut raw_lines: Vec<RawLine> = Vec::new();
        for g in raw_glyphs {
            // Find an existing line whose baseline matches within 0.40 * font_size AND isn't across a column jump
            // if a different line already matches closer in X? Wait: two columns at the same Y should stay separate
            // OR split during column detection! Keeping same-Y glyphs together first and splitting at wide gaps works cleanly.
            if let Some(last) = raw_lines.last_mut()
                && (g.baseline_y - last.baseline_y).abs()
                    <= 0.42 * last.font_size.max(g.font_size)
            {
                let n = last.glyphs.len() as f32;
                last.baseline_y = (last.baseline_y * n + g.baseline_y) / (n + 1.0);
                last.font_size = (last.font_size * n + g.font_size) / (n + 1.0);
                last.bounds = last.bounds.union(g.rect);
                last.glyphs.push(g);
            } else {
                let bounds = g.rect;
                let baseline_y = g.baseline_y;
                let font_size = g.font_size;
                raw_lines.push(RawLine {
                    glyphs: vec![g],
                    bounds,
                    baseline_y,
                    font_size,
                });
            }
        }

        // Split any raw line that bridges a wide column gutter (> 3.0 * font_size) into distinct segments
        // so left-column and right-column lines at identical Y coordinates can be ordered column-by-column!
        let mut segmented_lines: Vec<RawLine> = Vec::with_capacity(raw_lines.len());
        for mut line in raw_lines {
            line.glyphs.sort_by(|a, b| a.rect.min_x.total_cmp(&b.rect.min_x));
            let gap_threshold = (line.font_size * 2.8).max(18.0);
            let mut current_chunk: Vec<ExtractedGlyph> = Vec::new();
            for g in line.glyphs {
                if let Some(prev) = current_chunk.last()
                    && (g.rect.min_x - prev.rect.max_x) > gap_threshold
                {
                    segmented_lines.push(build_raw_line(current_chunk));
                    current_chunk = Vec::new();
                }
                current_chunk.push(g);
            }
            if !current_chunk.is_empty() {
                segmented_lines.push(build_raw_line(current_chunk));
            }
        }

        // Column-aware reading order: detect contiguous vertical bands of 2-column segments around page center.
        let ordered_lines = order_columns(segmented_lines, page_width);

        let mut full_text = String::new();
        let mut glyphs = Vec::new();
        let mut lines = Vec::new();

        for (line_idx_usize, line) in ordered_lines.into_iter().enumerate() {
            if line_idx_usize > 0 {
                full_text.push('\n');
            }
            let line_idx = line_idx_usize as u32;
            let span_start = glyphs.len();
            let mut line_bounds = line.bounds;

            for (i, g) in line.glyphs.iter().enumerate() {
                if i > 0 {
                    let prev = &line.glyphs[i - 1];
                    let gap = g.rect.min_x - prev.rect.max_x;
                    let space_thresh = 0.22 * prev.font_size.min(g.font_size);
                    let prev_ends_ws = prev.text.chars().last().is_some_and(char::is_whitespace);
                    let next_starts_ws = g.text.chars().next().is_some_and(char::is_whitespace);

                    if gap > space_thresh && !prev_ends_ws && !next_starts_ws {
                        let start_byte = full_text.len();
                        full_text.push(' ');
                        let end_byte = full_text.len();
                        let space_rect = NormRect::new(
                            prev.rect.max_x,
                            line.bounds.min_y,
                            g.rect.min_x.max(prev.rect.max_x + 0.5),
                            line.bounds.max_y,
                        );
                        glyphs.push(GlyphSpan {
                            byte_range: start_byte..end_byte,
                            rect: space_rect,
                            line_index: line_idx,
                            is_synthetic_space: true,
                        });
                    }
                }

                let start_byte = full_text.len();
                full_text.push_str(&g.text);
                let end_byte = full_text.len();
                line_bounds = line_bounds.union(g.rect);
                glyphs.push(GlyphSpan {
                    byte_range: start_byte..end_byte,
                    rect: g.rect,
                    line_index: line_idx,
                    is_synthetic_space: false,
                });
            }

            let span_end = glyphs.len();
            if span_end > span_start {
                lines.push(TextLine {
                    line_index: line_idx,
                    span_range: span_start..span_end,
                    bounds: line_bounds,
                    baseline_y: line.baseline_y,
                    font_size: line.font_size,
                });
            }
        }

        Self {
            page,
            full_text,
            glyphs,
            lines,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    pub fn hit_test(&self, pt: NormPoint) -> Option<usize> {
        if self.glyphs.is_empty() {
            return None;
        }

        // First pass: direct hit inside any glyph box (with slight vertical/horizontal tolerance).
        for (idx, span) in self.glyphs.iter().enumerate() {
            if span.rect.inflate(1.5).contains(pt) {
                return Some(idx);
            }
        }

        // Second pass: find closest line within vertical reach, then closest glyph on that line.
        let mut best_line: Option<(f32, &TextLine)> = None;
        for line in &self.lines {
            let dy = if pt.y < line.bounds.min_y {
                line.bounds.min_y - pt.y
            } else if pt.y > line.bounds.max_y {
                pt.y - line.bounds.max_y
            } else {
                0.0
            };
            if dy <= (line.font_size * 1.4).max(16.0) {
                let cx = line.bounds.center().x;
                let dx = if pt.x < line.bounds.min_x {
                    line.bounds.min_x - pt.x
                } else if pt.x > line.bounds.max_x {
                    pt.x - line.bounds.max_x
                } else {
                    0.0
                };
                // Prefer lines whose X span covers pt.x, then vertical proximity.
                let score = dy * 2.0 + dx * 0.5 + (pt.x - cx).abs() * 0.001;
                if best_line.is_none_or(|(best_score, _)| score < best_score) {
                    best_line = Some((score, line));
                }
            }
        }

        let (_, line) = best_line?;
        let mut best_glyph: Option<(f32, usize)> = None;
        for idx in line.span_range.clone() {
            let rect = self.glyphs[idx].rect;
            let dx = if pt.x < rect.min_x {
                rect.min_x - pt.x
            } else if pt.x > rect.max_x {
                pt.x - rect.max_x
            } else {
                0.0
            };
            if best_glyph.is_none_or(|(d, _)| dx < d) {
                best_glyph = Some((dx, idx));
            }
        }
        best_glyph.map(|(_, idx)| idx)
    }

    pub fn word_range_at(&self, glyph_idx: usize) -> Option<(usize, usize)> {
        let span = self.glyphs.get(glyph_idx)?;
        let line = self.lines.get(span.line_index as usize)?;
        let is_word = |idx: usize| -> bool {
            let s = &self.full_text[self.glyphs[idx].byte_range.clone()];
            s.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                && !s.is_empty()
        };
        if !is_word(glyph_idx) {
            return Some((glyph_idx, glyph_idx));
        }
        let mut start = glyph_idx;
        while start > line.span_range.start && is_word(start - 1) {
            start -= 1;
        }
        let mut end = glyph_idx;
        while end + 1 < line.span_range.end && is_word(end + 1) {
            end += 1;
        }
        Some((start, end))
    }

    pub fn line_range_at(&self, glyph_idx: usize) -> Option<(usize, usize)> {
        let span = self.glyphs.get(glyph_idx)?;
        let line = self.lines.get(span.line_index as usize)?;
        Some((line.span_range.start, line.span_range.end.saturating_sub(1)))
    }

    pub fn slice_text(&self, start_idx: usize, end_idx: usize) -> String {
        if self.glyphs.is_empty() {
            return String::new();
        }
        let lo = start_idx.min(end_idx).min(self.glyphs.len() - 1);
        let hi = start_idx.max(end_idx).min(self.glyphs.len() - 1);
        let start_byte = self.glyphs[lo].byte_range.start;
        let end_byte = self.glyphs[hi].byte_range.end;
        self.full_text[start_byte..end_byte].to_string()
    }

    /// Merges contiguous selected glyphs on each line into smooth, uniform line-height quads.
    pub fn selection_quads(&self, start_idx: usize, end_idx: usize) -> Vec<NormQuad> {
        if self.glyphs.is_empty() {
            return Vec::new();
        }
        let lo = start_idx.min(end_idx).min(self.glyphs.len() - 1);
        let hi = start_idx.max(end_idx).min(self.glyphs.len() - 1);

        let mut quads = Vec::new();
        let mut cur_line: Option<u32> = None;
        let mut cur_min_x = 0.0_f32;
        let mut cur_max_x = 0.0_f32;
        let mut cur_min_y = 0.0_f32;
        let mut cur_max_y = 0.0_f32;

        for span in &self.glyphs[lo..=hi] {
            let line = &self.lines[span.line_index as usize];
            if cur_line == Some(span.line_index) {
                cur_min_x = cur_min_x.min(span.rect.min_x);
                cur_max_x = cur_max_x.max(span.rect.max_x);
            } else {
                if cur_line.is_some() && cur_max_x > cur_min_x {
                    quads.push(NormQuad::from_rect(NormRect::new(
                        cur_min_x, cur_min_y, cur_max_x, cur_max_y,
                    )));
                }
                cur_line = Some(span.line_index);
                cur_min_x = span.rect.min_x;
                cur_max_x = span.rect.max_x;
                cur_min_y = line.bounds.min_y;
                cur_max_y = line.bounds.max_y;
            }
        }
        if cur_line.is_some() && cur_max_x > cur_min_x {
            quads.push(NormQuad::from_rect(NormRect::new(
                cur_min_x, cur_min_y, cur_max_x, cur_max_y,
            )));
        }
        quads
    }
}

fn build_raw_line(glyphs: Vec<ExtractedGlyph>) -> RawLine {
    let mut bounds = glyphs[0].rect;
    let mut sum_y = 0.0_f32;
    let mut sum_fs = 0.0_f32;
    for g in &glyphs {
        bounds = bounds.union(g.rect);
        sum_y += g.baseline_y;
        sum_fs += g.font_size;
    }
    let n = glyphs.len() as f32;
    RawLine {
        glyphs,
        bounds,
        baseline_y: sum_y / n,
        font_size: sum_fs / n,
    }
}

/// Orders contiguous 2-column bands (where lines lie cleanly left or right of page center)
/// so Left Column reads completely before Right Column while preserving full-width headers/footers.
fn order_columns(mut lines: Vec<RawLine>, page_width: f32) -> Vec<RawLine> {
    if lines.len() < 4 || page_width <= 1.0 {
        return lines;
    }

    lines.sort_by(|a, b| {
        a.baseline_y
            .total_cmp(&b.baseline_y)
            .then_with(|| a.bounds.min_x.total_cmp(&b.bounds.min_x))
    });

    let gutter_x = page_width * 0.5;
    let tolerance = 6.0;

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum BandSide {
        FullWidth,
        Left,
        Right,
    }

    let classify = |l: &RawLine| -> BandSide {
        if l.bounds.max_x <= gutter_x + tolerance && l.bounds.width() < page_width * 0.50 {
            BandSide::Left
        } else if l.bounds.min_x >= gutter_x - tolerance && l.bounds.width() < page_width * 0.50 {
            BandSide::Right
        } else {
            BandSide::FullWidth
        }
    };

    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if classify(&lines[i]) == BandSide::FullWidth {
            result.push(lines[i].clone());
            i += 1;
            continue;
        }

        // Collect contiguous non-full-width block.
        let mut left_col = Vec::new();
        let mut right_col = Vec::new();
        while i < lines.len() {
            match classify(&lines[i]) {
                BandSide::Left => left_col.push(lines[i].clone()),
                BandSide::Right => right_col.push(lines[i].clone()),
                BandSide::FullWidth => break,
            }
            i += 1;
        }

        // Only treat as 2-column when BOTH sides have content; otherwise keep original Y order.
        if !left_col.is_empty() && !right_col.is_empty() {
            result.extend(left_col);
            result.extend(right_col);
        } else {
            // Re-merge by Y if only one side appeared (e.g. short left-aligned paragraph).
            let mut single = left_col;
            single.extend(right_col);
            single.sort_by(|a, b| a.baseline_y.total_cmp(&b.baseline_y));
            result.extend(single);
        }
    }

    result
}
