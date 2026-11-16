use std::sync::Arc;

use pinleaf_core::PageIndex;
use regex::Regex;

use crate::text_layout::{PageTextLayout, TextLine};

#[derive(Clone, Debug, PartialEq)]
pub struct SmartTarget {
    pub label: String,
    pub page: PageIndex,
    pub top_norm_y: f32,
    pub snippet: String,
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

    // 1. Named structural references: Figure 1 / Fig. 1 / Table 2 / Theorem 1 / Section 3 ...
    let named_re = Regex::new(
        r"(?i)\b(Fig(?:ure|s)?\.?|Tables?|Tab\.?|Theorems?|Thm\.?|Lemmas?|Propositions?|Corollar(?:y|ies)|Definitions?|Algorithms?|Sections?|Sec\.?|Appendix)\s*([0-9]+[A-Za-z]?(?:\.[0-9]+)*|[A-Z](?:\.[0-9]+)*)",
    )
    .ok()?;
    for caps in named_re.captures_iter(line_text) {
        let full = caps.get(0)?;
        if overlaps_cursor(full.start(), full.end(), cursor_offset) {
            let kind = canonical_kind(caps.get(1)?.as_str());
            let ident = caps.get(2)?.as_str();
            if let Some(target) = find_named_target(
                source_page,
                line.line_index,
                kind,
                ident,
                layouts,
            ) {
                return Some(target);
            }
        }
    }

    // 2. Bracketed bibliography citations: [1], [2, 5], [3-6]
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
                && let Some(target) = find_citation_target(source_page, line.line_index, cite_num, layouts)
            {
                return Some(target);
            }
        }
    }

    // 3. Equation references: Eq. (1), Equation (2.3)
    let eq_re = Regex::new(r"(?i)\bEq(?:uation|s)?\.?\s*\(([0-9]+(?:\.[0-9]+)*)\)").ok()?;
    for caps in eq_re.captures_iter(line_text) {
        let full = caps.get(0)?;
        if overlaps_cursor(full.start(), full.end(), cursor_offset) {
            let eq_num = caps.get(1)?.as_str();
            if let Some(target) = find_equation_target(source_page, line.line_index, eq_num, layouts)
            {
                return Some(target);
            }
        }
    }

    None
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
    }
}
