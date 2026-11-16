use std::ops::Range;
use std::sync::Arc;

use pinleaf_core::{NormRect, PageIndex};
use regex::Regex;

use crate::text_layout::{PageTextLayout, TextLine};

#[derive(Clone, Debug, PartialEq)]
pub struct SmartTarget {
    pub label: String,
    pub page: PageIndex,
    pub top_norm_y: f32,
    pub snippet: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineSmartRef {
    pub rect: NormRect,
    pub target: SmartTarget,
}

struct LineRefMatch {
    line_byte_range: Range<usize>,
    target: SmartTarget,
}

fn collect_line_matches(
    source_page: PageIndex,
    line_idx: u32,
    line_text: &str,
    layouts: &[Arc<PageTextLayout>],
) -> Vec<LineRefMatch> {
    let mut out = Vec::new();

    // 1. Named structural references: Figure 1 / Fig. 1 / Table 2 / Theorem 1 / Section 3 ...
    if let Ok(named_re) = Regex::new(
        r"(?i)\b(Fig(?:ure|s)?\.?|Tables?|Tab\.?|Theorems?|Thm\.?|Lemmas?|Propositions?|Corollar(?:y|ies)|Definitions?|Algorithms?|Sections?|Sec\.?|Appendix)\s*([0-9]+[A-Za-z]?(?:\.[0-9]+)*|[A-Z](?:\.[0-9]+)*)",
    ) {
        for caps in named_re.captures_iter(line_text) {
            if let (Some(full), Some(kind_m), Some(id_m)) = (caps.get(0), caps.get(1), caps.get(2)) {
                let kind = canonical_kind(kind_m.as_str());
                if let Some(target) = find_named_target(
                    source_page,
                    line_idx,
                    kind,
                    id_m.as_str(),
                    layouts,
                ) {
                    out.push(LineRefMatch {
                        line_byte_range: full.start()..full.end(),
                        target,
                    });
                }
            }
        }
    }

    // 2. Bracketed bibliography citations: [1], [2, 5], [3-6]
    if let (Ok(cite_re), Ok(num_re)) = (Regex::new(r"\[([0-9][0-9,\s\-–]{0,24})\]"), Regex::new(r"\d+")) {
        for caps in cite_re.captures_iter(line_text) {
            if let (Some(full), Some(inner)) = (caps.get(0), caps.get(1)) {
                let first_num = num_re.find(inner.as_str()).map(|m| m.as_str());
                if let Some(cite_num) = first_num
                    && let Some(target) =
                        find_citation_target(source_page, line_idx, cite_num, layouts)
                {
                    out.push(LineRefMatch {
                        line_byte_range: full.start()..full.end(),
                        target,
                    });
                }
            }
        }
    }

    // 3. Equation references: Eq. (1), Equation (2.3)
    if let Ok(eq_re) = Regex::new(r"(?i)\bEq(?:uation|s)?\.?\s*\(([0-9]+(?:\.[0-9]+)*)\)") {
        for caps in eq_re.captures_iter(line_text) {
            if let (Some(full), Some(num_m)) = (caps.get(0), caps.get(1))
                && let Some(target) =
                    find_equation_target(source_page, line_idx, num_m.as_str(), layouts)
            {
                out.push(LineRefMatch {
                    line_byte_range: full.start()..full.end(),
                    target,
                });
            }
        }
    }

    out
}

/// Scans all lines on `page` overlapping `[y_min, y_max]` and collects resolved Smart Jump references
/// paired with their bounding box for keyboard hint overlays.
pub fn collect_page_smart_refs(
    page: PageIndex,
    y_min: f32,
    y_max: f32,
    layouts: &[Arc<PageTextLayout>],
) -> Vec<LineSmartRef> {
    let Some(source_layout) = layouts.iter().find(|l| l.page == page) else {
        return Vec::new();
    };
    let mut refs = Vec::new();
    for line in &source_layout.lines {
        if line.bounds.max_y < y_min || line.bounds.min_y > y_max {
            continue;
        }
        let Some(first_glyph) = source_layout.glyphs.get(line.span_range.start) else {
            continue;
        };
        let Some(last_idx) = line.span_range.end.checked_sub(1) else {
            continue;
        };
        let Some(last_glyph) = source_layout.glyphs.get(last_idx) else {
            continue;
        };
        let line_start_byte = first_glyph.byte_range.start;
        let Some(line_text) = source_layout
            .full_text
            .get(line_start_byte..last_glyph.byte_range.end)
        else {
            continue;
        };

        for matched in collect_line_matches(page, line.line_index, line_text, layouts) {
            let abs_start = line_start_byte + matched.line_byte_range.start;
            let abs_end = line_start_byte + matched.line_byte_range.end;
            let mut rect: Option<NormRect> = None;
            for span in &source_layout.glyphs[line.span_range.clone()] {
                if span.byte_range.end > abs_start && span.byte_range.start < abs_end {
                    rect = Some(match rect {
                        Some(acc) => acc.union(span.rect),
                        None => span.rect,
                    });
                }
            }
            if let Some(r) = rect {
                refs.push(LineSmartRef {
                    rect: r,
                    target: matched.target,
                });
            }
        }
    }
    refs
}

/// Resolves unlinked academic references (`Figure 1`, `Theorem 2`, `Table 1`, `Eq. (3)`, `[12]`)
/// around `glyph_idx` on `source_page` against extracted text lines across the document.
pub fn resolve_smart_jump_at(
    source_page: PageIndex,
    glyph_idx: usize,
    layouts: &[Arc<PageTextLayout>],
) -> Option<SmartTarget> {
    let source_layout = layouts.iter().find(|l| l.page == source_page)?;
    let span = source_layout.glyphs.get(glyph_idx)?;
    let line = source_layout.lines.get(span.line_index as usize)?;
    let line_first = source_layout.glyphs.get(line.span_range.start)?;
    let line_last = source_layout
        .glyphs
        .get(line.span_range.end.checked_sub(1)?)?;

    let line_start_byte = line_first.byte_range.start;
    let line_text = source_layout
        .full_text
        .get(line_start_byte..line_last.byte_range.end)?;
    let cursor_offset = ((span.byte_range.start + span.byte_range.end) / 2)
        .saturating_sub(line_start_byte)
        .min(line_text.len());

    // Preserve multi-citation cursor precision ([2, 5] picks the specific number near the cursor).
    let cite_re = Regex::new(r"\[([0-9][0-9,\s\-–]{0,24})\]").ok()?;
    for caps in cite_re.captures_iter(line_text) {
        let full = caps.get(0)?;
        if overlaps_cursor(full.start(), full.end(), cursor_offset) {
            let inner = caps.get(1)?;
            let num_re = Regex::new(r"\d+").ok()?;
            let mut best_num: Option<(usize, &str)> = None;
            for nm in num_re.find_iter(inner.as_str()) {
                let abs_mid = inner.start() + (nm.start() + nm.end()) / 2;
                let dist = abs_mid.abs_diff(cursor_offset);
                if best_num.is_none_or(|(bd, _)| dist < bd) {
                    best_num = Some((dist, nm.as_str()));
                }
            }
            if let Some((_, cite_num)) = best_num
                && let Some(target) =
                    find_citation_target(source_page, line.line_index, cite_num, layouts)
            {
                return Some(target);
            }
        }
    }

    collect_line_matches(source_page, line.line_index, line_text, layouts)
        .into_iter()
        .find(|m| overlaps_cursor(m.line_byte_range.start, m.line_byte_range.end, cursor_offset))
        .map(|m| m.target)
}

fn overlaps_cursor(start: usize, end: usize, cursor: usize) -> bool {
    cursor + 3 >= start && cursor <= end + 3
}

fn canonical_kind(raw: &str) -> &'static str {
    let lower = raw.trim_end_matches('.').to_ascii_lowercase();
    if lower.starts_with("fig") {
        "Figure"
    } else if lower.starts_with("tab") {
        "Table"
    } else if lower.starts_with("thm") || lower.starts_with("theorem") {
        "Theorem"
    } else if lower.starts_with("lemma") {
        "Lemma"
    } else if lower.starts_with("prop") {
        "Proposition"
    } else if lower.starts_with("corol") {
        "Corollary"
    } else if lower.starts_with("def") {
        "Definition"
    } else if lower.starts_with("algo") {
        "Algorithm"
    } else if lower.starts_with("sec") {
        "Section"
    } else {
        "Appendix"
    }
}

fn kind_pattern(kind: &str) -> &'static str {
    match kind {
        "Figure" => r"(?:Fig(?:ure)?\.?)",
        "Table" => r"(?:Table|Tab\.?)",
        "Theorem" => r"(?:Theorem|Thm\.?)",
        "Lemma" => r"Lemma",
        "Proposition" => r"Proposition",
        "Corollary" => r"Corollary",
        "Definition" => r"Definition",
        "Algorithm" => r"Algorithm",
        "Section" => r"(?:Section|Sec\.?)",
        _ => r"Appendix",
    }
}

fn find_named_target(
    source_page: PageIndex,
    source_line_idx: u32,
    kind: &str,
    ident: &str,
    layouts: &[Arc<PageTextLayout>],
) -> Option<SmartTarget> {
    let pat = format!(
        r"(?i)^\s*(?:{}\s*{}\b|{}(?:\s*{}\b)\s*[:.(])",
        kind_pattern(kind),
        regex::escape(ident),
        kind_pattern(kind),
        regex::escape(ident),
    );
    let strong_re = Regex::new(&pat).ok()?;
    let soft_pat = format!(
        r"(?i)\b{}\s*{}(?:\s*[:.(]|\s|$)",
        kind_pattern(kind),
        regex::escape(ident),
    );
    let soft_re = Regex::new(&soft_pat).ok()?;

    let mut best: Option<(i32, SmartTarget)> = None;
    for layout in layouts {
        for line in &layout.lines {
            if layout.page == source_page && line.line_index == source_line_idx {
                continue;
            }
            let Some(txt) = get_line_str(layout, line) else {
                continue;
            };
            let trimmed = txt.trim();
            let score = if strong_re.is_match(trimmed) {
                100 + if layout.page != source_page { 20 } else { 0 }
                    + (line.font_size.round() as i32)
            } else if soft_re.is_match(trimmed)
                && !trimmed.starts_with("See ")
                && !trimmed.starts_with("see ")
            {
                40 + if layout.page != source_page { 10 } else { 0 }
            } else {
                continue;
            };

            let cand = SmartTarget {
                label: format!("{kind} {ident}"),
                page: layout.page,
                top_norm_y: line.bounds.min_y,
                snippet: trimmed.chars().take(72).collect(),
            };
            if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
                best = Some((score, cand));
            }
        }
    }

    best.map(|(_, t)| t)
}

fn find_citation_target(
    source_page: PageIndex,
    source_line_idx: u32,
    cite_num: &str,
    layouts: &[Arc<PageTextLayout>],
) -> Option<SmartTarget> {
    let start_pat = format!(r"^\s*(?:\[{n}\]|{n}\.\s+\S)", n = regex::escape(cite_num));
    let re = Regex::new(&start_pat).ok()?;

    // Track first bibliography/references page so entries after it get highest score.
    let ref_page = layouts.iter().find_map(|layout| {
        layout.lines.iter().find_map(|line| {
            let t = get_line_str(layout, line)?.trim();
            let lower = t.to_ascii_lowercase();
            if lower == "references" || lower == "bibliography" || lower.starts_with("references ")
            {
                Some(layout.page)
            } else {
                None
            }
        })
    });

    let mut best: Option<(i32, SmartTarget)> = None;
    for layout in layouts.iter().rev() {
        for line in &layout.lines {
            if layout.page == source_page && line.line_index == source_line_idx {
                continue;
            }
            let Some(txt) = get_line_str(layout, line) else {
                continue;
            };
            let trimmed = txt.trim();
            if !re.is_match(trimmed) {
                continue;
            }
            let mut score = 50;
            if ref_page.is_some_and(|rp| layout.page >= rp) {
                score += 100;
            }
            if trimmed.starts_with('[') {
                score += 20;
            }
            let cand = SmartTarget {
                label: format!("[{cite_num}]"),
                page: layout.page,
                top_norm_y: line.bounds.min_y,
                snippet: trimmed.chars().take(72).collect(),
            };
            if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
                best = Some((score, cand));
            }
        }
    }

    best.map(|(_, t)| t)
}

fn find_equation_target(
    source_page: PageIndex,
    source_line_idx: u32,
    eq_num: &str,
    layouts: &[Arc<PageTextLayout>],
) -> Option<SmartTarget> {
    let end_pat = format!(r"\({}\)\s*$", regex::escape(eq_num));
    let any_pat = format!(r"\({}\)", regex::escape(eq_num));
    let end_re = Regex::new(&end_pat).ok()?;
    let any_re = Regex::new(&any_pat).ok()?;

    let mut best: Option<(i32, SmartTarget)> = None;
    for layout in layouts {
        for line in &layout.lines {
            if layout.page == source_page && line.line_index == source_line_idx {
                continue;
            }
            let Some(txt) = get_line_str(layout, line) else {
                continue;
            };
            let trimmed = txt.trim();
            let score = if end_re.is_match(trimmed) {
                100
            } else if any_re.is_match(trimmed)
                && !trimmed.to_ascii_lowercase().contains("eq.")
                && !trimmed.to_ascii_lowercase().contains("equation")
            {
                40
            } else {
                continue;
            };
            let cand = SmartTarget {
                label: format!("Eq. ({eq_num})"),
                page: layout.page,
                top_norm_y: line.bounds.min_y,
                snippet: trimmed.chars().take(72).collect(),
            };
            if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
                best = Some((score, cand));
            }
        }
    }
    best.map(|(_, t)| t)
}

fn get_line_str<'a>(layout: &'a PageTextLayout, line: &TextLine) -> Option<&'a str> {
    let first = layout.glyphs.get(line.span_range.start)?;
    let last = layout.glyphs.get(line.span_range.end.checked_sub(1)?)?;
    layout
        .full_text
        .get(first.byte_range.start..last.byte_range.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo_fixture::build_demo_pdf_bytes;
    use crate::{PdfDocumentHandle, extract_page_text};

    #[test]
    fn resolves_figure_theorem_and_citation_smart_jumps_on_demo_pdf() {
        let bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(bytes).unwrap();
        let layouts: Vec<Arc<PageTextLayout>> = (0..handle.page_count())
            .map(|i| Arc::new(extract_page_text(&handle, PageIndex(i as u32)).unwrap()))
            .collect();

        let p1 = &layouts[0];
        let glyph_at_token = |token: &str| -> usize {
            let byte_pos = p1.full_text.find(token).expect("token exists on page 1");
            let mid = byte_pos + token.len() / 2;
            p1.glyphs
                .iter()
                .position(|g| g.byte_range.start <= mid && mid <= g.byte_range.end)
                .expect("glyph at token")
        };

        let fig = resolve_smart_jump_at(PageIndex(0), glyph_at_token("Figure 1"), &layouts)
            .expect("Figure 1 smart target");
        assert_eq!(fig.page, PageIndex(2));
        assert_eq!(fig.label, "Figure 1");

        let thm = resolve_smart_jump_at(PageIndex(0), glyph_at_token("Theorem 1"), &layouts)
            .expect("Theorem 1 smart target");
        assert_eq!(thm.page, PageIndex(1));
        assert_eq!(thm.label, "Theorem 1");

        let cite = resolve_smart_jump_at(PageIndex(0), glyph_at_token("[1]"), &layouts)
            .expect("[1] smart target");
        assert_eq!(cite.page, PageIndex(3));
        assert_eq!(cite.label, "[1]");

        let refs = collect_page_smart_refs(PageIndex(0), 0.0, 792.0, &layouts);
        let labels: Vec<&str> = refs.iter().map(|r| r.target.label.as_str()).collect();
        assert!(labels.contains(&"Figure 1"), "got {labels:?}");
        assert!(labels.contains(&"Theorem 1"), "got {labels:?}");
        assert!(labels.contains(&"[1]"), "got {labels:?}");
    }
}
