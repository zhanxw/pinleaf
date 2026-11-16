use std::sync::Arc;

use pinleaf_core::{NormQuad, PageIndex};
use regex::RegexBuilder;

use crate::text_layout::PageTextLayout;

const MAX_SEARCH_MATCHES: usize = 2000;

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub page: PageIndex,
    pub quads: Vec<NormQuad>,
    pub snippet: String,
    pub top_norm_y: f32,
}

/// Searches extracted reading-order text across all pages, supporting case-insensitive
/// substring search and regex mode (explicit `use_regex = true` or `re:` prefix).
pub fn search_layouts(
    layouts: &[Arc<PageTextLayout>],
    query: &str,
    use_regex: bool,
) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let (pattern, regex_mode) = if let Some(rest) = query.strip_prefix("re:") {
        (rest, true)
    } else {
        (query, use_regex)
    };
    if pattern.is_empty() {
        return Vec::new();
    }

    let compiled = if regex_mode {
        RegexBuilder::new(pattern).case_insensitive(true).build()
    } else {
        RegexBuilder::new(&regex::escape(pattern))
            .case_insensitive(true)
            .build()
    };
    let Ok(re) = compiled else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    for layout in layouts {
        if layout.glyphs.is_empty() {
            continue;
        }
        for m in re.find_iter(&layout.full_text) {
            if m.start() == m.end() {
                continue;
            }
            let first_glyph = layout
                .glyphs
                .partition_point(|g| g.byte_range.end <= m.start());
            let last_glyph = layout
                .glyphs
                .partition_point(|g| g.byte_range.start < m.end())
                .saturating_sub(1);
            if first_glyph <= last_glyph && last_glyph < layout.glyphs.len() {
                let quads = layout.selection_quads(first_glyph, last_glyph);
                if quads.is_empty() {
                    continue;
                }
                let top_norm_y = quads.first().map(|q| q.bounding_rect().min_y).unwrap_or(0.0);
                matches.push(SearchMatch {
                    page: layout.page,
                    quads,
                    snippet: build_snippet(&layout.full_text, m.start(), m.end()),
                    top_norm_y,
                });
                if matches.len() >= MAX_SEARCH_MATCHES {
                    return matches;
                }
            }
        }
    }

    matches
}

fn build_snippet(text: &str, start_byte: usize, end_byte: usize) -> String {
    let lo_target = start_byte.saturating_sub(24);
    let hi_target = (end_byte + 28).min(text.len());
    let lo = floor_char_boundary(text, lo_target);
    let hi = ceil_char_boundary(text, hi_target);
    let mut s = String::new();
    if lo > 0 {
        s.push_str("...");
    }
    s.push_str(&text[lo..hi].replace('\n', " "));
    if hi < text.len() {
        s.push_str("...");
    }
    s
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo_fixture::build_demo_pdf_bytes;
    use crate::{PdfDocumentHandle, extract_page_text};

    #[test]
    fn plain_and_regex_search_find_matches_with_quads() {
        let bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(bytes).unwrap();
        let layouts: Vec<Arc<PageTextLayout>> = (0..handle.page_count())
            .map(|i| Arc::new(extract_page_text(&handle, PageIndex(i as u32)).unwrap()))
            .collect();

        let plain = search_layouts(&layouts, "theorem 1", false);
        assert!(plain.len() >= 3);
        assert!(plain.iter().any(|m| m.page == PageIndex(1)));
        assert!(plain.iter().any(|m| m.page == PageIndex(3)));
        assert!(plain.iter().all(|m| !m.quads.is_empty()));

        let regex = search_layouts(&layouts, r"Viewport #[1-3]", true);
        assert_eq!(regex.len(), 3);
        assert!(regex.iter().all(|m| m.page == PageIndex(2)));

        let prefix_re = search_layouts(&layouts, r"re:Page \d of 5", false);
        assert_eq!(prefix_re.len(), 5);
    }
}
