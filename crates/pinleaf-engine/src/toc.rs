use lopdf::Document;
use pinleaf_core::PageIndex;
use regex::Regex;

use crate::document::PdfDocumentHandle;
use crate::text_layout::extract_page_text;

#[derive(Clone, Debug, PartialEq)]
pub struct TocEntry {
    pub level: usize,
    pub title: String,
    pub page: PageIndex,
    pub top_norm_y: f32,
    pub is_inferred: bool,
}

const MAX_AUTO_TOC_PAGES: usize = 120;

/// Extracts embedded PDF `/Outlines` via `lopdf`, falling back to font-size + academic
/// section-pattern heading inference when a document has no native outline tree.
pub fn extract_toc(pdf_bytes: &[u8], handle: &PdfDocumentHandle) -> Vec<TocEntry> {
    let page_count = handle.page_count();
    if page_count == 0 {
        return Vec::new();
    }

    if let Ok(doc) = Document::load_mem(pdf_bytes)
        && let Ok(toc) = doc.get_toc()
        && !toc.toc.is_empty()
    {
        return toc
            .toc
            .into_iter()
            .filter_map(|t| {
                let title = t.title.trim().to_string();
                if title.is_empty() {
                    return None;
                }
                let idx = t.page.saturating_sub(1).min(page_count - 1) as u32;
                Some(TocEntry {
                    level: t.level.max(1),
                    title,
                    page: PageIndex(idx),
                    top_norm_y: 0.0,
                    is_inferred: false,
                })
            })
            .collect();
    }

    infer_auto_toc(handle)
}

fn infer_auto_toc(handle: &PdfDocumentHandle) -> Vec<TocEntry> {
    let pages_to_scan = handle.page_count().min(MAX_AUTO_TOC_PAGES);
    let mut layouts = Vec::with_capacity(pages_to_scan);
    let mut sample_sizes = Vec::new();

    for i in 0..pages_to_scan {
        if let Ok(layout) = extract_page_text(handle, PageIndex(i as u32)) {
            for line in &layout.lines {
                if let Some(txt) = line_text(&layout, line) {
                    if txt.chars().count() >= 15 {
                        sample_sizes.push(line.font_size);
                    }
                }
            }
            layouts.push(layout);
        }
    }

    let body_fs = if sample_sizes.is_empty() {
        10.0
    } else {
        sample_sizes.sort_by(|a, b| a.total_cmp(b));
        sample_sizes[sample_sizes.len() / 2].max(6.0)
    };

    let heading_re = Regex::new(
        r"^(?:Abstract\b.*|Introduction\b.*|Conclusion\b.*|References\b.*|Bibliography\b.*|Appendix(?:\s+[A-Z0-9]+)?\b.*|Section\s+\d+.*|\d+(?:\.\d+)*\.?\s+[A-Z][A-Za-z0-9\s\-:()]+|Theorem\s+\d+.*|Proof\s+of\s+Theorem\s+\d+.*|Figure\s+\d+:.*)$",
    )
    .expect("valid heading regex");
    let footer_re = Regex::new(r"^(?i:page\s+\d+(?:\s+of\s+\d+)?|\d+)$").expect("valid footer regex");

    let mut entries = Vec::new();
    for layout in &layouts {
        for line in &layout.lines {
            let Some(raw_txt) = line_text(layout, line) else {
                continue;
            };
            let title = raw_txt.trim();
            let char_len = title.chars().count();
            if !(4..=110).contains(&char_len) || footer_re.is_match(title) {
                continue;
            }

            let level = if line.font_size >= body_fs * 1.28 {
                Some(1)
            } else if line.font_size >= body_fs * 1.08 {
                Some(2)
            } else if heading_re.is_match(title) {
                let top_sec = title.starts_with("Abstract")
                    || title.starts_with("References")
                    || title.starts_with("Section")
                    || (title.chars().next().is_some_and(|c| c.is_ascii_digit())
                        && !title.split_whitespace().next().unwrap_or("").contains('.'));
                Some(if top_sec { 1 } else { 2 })
            } else {
                None
            };

            if let Some(level) = level {
                entries.push(TocEntry {
                    level,
                    title: title.to_string(),
                    page: layout.page,
                    top_norm_y: line.bounds.min_y,
                    is_inferred: true,
                });
            }
        }
    }

    entries
}

fn line_text<'a>(
    layout: &'a crate::text_layout::PageTextLayout,
    line: &crate::text_layout::TextLine,
) -> Option<&'a str> {
    let first = layout.glyphs.get(line.span_range.start)?;
    let last = layout.glyphs.get(line.span_range.end.checked_sub(1)?)?;
    layout.full_text.get(first.byte_range.start..last.byte_range.end)
}

pub fn filter_toc<'a>(entries: &'a [TocEntry], query: &str) -> Vec<&'a TocEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return entries.iter().collect();
    }
    let tokens: Vec<String> = trimmed
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .collect();
    entries
        .iter()
        .filter(|entry| {
            let title_l = entry.title.to_ascii_lowercase();
            let page_l = format!("p.{}", entry.page.one_based());
            tokens
                .iter()
                .all(|tok| title_l.contains(tok) || page_l.contains(tok))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::demo_fixture::build_demo_pdf_bytes;

    #[test]
    fn infers_auto_toc_for_demo_pdf_and_filters() {
        let bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(bytes.clone()).unwrap();
        let toc = extract_toc(&bytes, &handle);

        assert!(toc.iter().all(|e| e.is_inferred));
        let titles: Vec<&str> = toc.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("Freeze-and-Branch")), "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("1. Introduction")), "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("Section 3")), "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("Figure 1")), "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("Proof of Theorem 1")), "{titles:?}");

        let filtered = filter_toc(&toc, "theorem p.4");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].page, PageIndex(3));
    }
}
