pub mod demo_fixture;
pub mod document;
pub mod links;
pub mod rasterizer;
pub mod search;
pub mod smart_jump;
pub mod synctex;
pub mod text_device;
pub mod text_layout;
pub mod toc;

pub use document::{EngineError, PageMeta, PdfDocumentHandle};
pub use links::{LinkRegion, LinkTarget, extract_all_links};
pub use rasterizer::{BgraFrame, extract_page_image_rects, render_page_bgra};
pub use search::{SearchMatch, search_layouts};
pub use smart_jump::{LineSmartRef, SmartTarget, collect_page_smart_refs, resolve_smart_jump_at};
pub use synctex::{SyncTexMap, SyncTexPdfLoc, SyncTexSourceLoc};
pub use text_layout::{GlyphSpan, PageTextLayout, TextLine, extract_page_text};
pub use toc::{TocEntry, extract_toc, filter_toc};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pinleaf_core::{NormPoint, PageIndex};

    use crate::demo_fixture::build_demo_pdf_bytes;
    use crate::{
        LinkTarget, PdfDocumentHandle, extract_all_links, extract_page_text, render_page_bgra,
    };

    #[test]
    fn demo_pdf_renders_all_pages_and_skips_annotations() {
        let bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(bytes).expect("demo PDF should parse");
        assert_eq!(handle.page_count(), 5);

        let p5 = handle.page_meta(PageIndex(4)).unwrap();
        assert!((p5.norm_width - 792.0).abs() < 1e-3);
        assert!((p5.norm_height - 612.0).abs() < 1e-3);

        for i in 0..5 {
            let frame = render_page_bgra(&handle, PageIndex(i), 1.0).expect("page renders");
            assert!(frame.width > 100 && frame.height > 100);
            assert_eq!(frame.pixels_bgra.len(), (frame.width * frame.height * 4) as usize);
            let non_white = frame
                .pixels_bgra
                .chunks_exact(4)
                .filter(|px| px[0] < 250 || px[1] < 250 || px[2] < 250)
                .count();
            assert!(non_white > 100, "page {i} had only {non_white} non-white pixels");

            // On Page 1, the pre-existing highlight covers x=52..405, y_pdf=661..676 -> y_norm=116..131.
            // Check empty margin inside that box at x=53, y_norm=124 (just left of text at x=54) is pure white (not yellow-baked!).
            if i == 0 {
                let idx = (124 * frame.width as usize + 53) * 4;
                let [b, g, r, a] = [
                    frame.pixels_bgra[idx],
                    frame.pixels_bgra[idx + 1],
                    frame.pixels_bgra[idx + 2],
                    frame.pixels_bgra[idx + 3],
                ];
                assert_eq!([b, g, r, a], [255, 255, 255, 255]);
            }
        }
    }

    #[test]
    fn two_column_reading_order_rotated_text_and_hit_testing() {
        let bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(bytes).unwrap();

        // Page 1: Left column ("1. Introduction" .. "child panel") must precede Right column ("2. Core Invariants").
        let p1_text = extract_page_text(&handle, PageIndex(0)).unwrap();
        let intro_pos = p1_text
            .full_text
            .find("1. Introduction")
            .expect("missing 1. Introduction");
        let child_pos = p1_text
            .full_text
            .find("child panel")
            .expect("missing child panel");
        let core_pos = p1_text
            .full_text
            .find("2. Core Invariants")
            .expect("missing 2. Core Invariants");
        assert!(intro_pos < child_pos && child_pos < core_pos);

        // Page 2: Synthetic space recovery + hit testing + merged selection quads.
        let p2_text = extract_page_text(&handle, PageIndex(1)).unwrap();
        assert!(
            p2_text.full_text.contains("Theorem 1 (Multi-View Reading Invariant)"),
            "got text:\n{}",
            p2_text.full_text
        );

        let thm_line = p2_text
            .lines
            .iter()
            .find(|l| {
                let first = &p2_text.glyphs[l.span_range.start];
                let last = &p2_text.glyphs[l.span_range.end - 1];
                let slice = &p2_text.full_text[first.byte_range.start..last.byte_range.end];
                slice.contains("Theorem 1")
            })
            .expect("Theorem 1 line found");
        let mid_pt = NormPoint::new(thm_line.bounds.min_x + 20.0, thm_line.bounds.center().y);
        let hit_idx = p2_text.hit_test(mid_pt).expect("hit_test should match glyph");
        let (w_start, w_end) = p2_text.word_range_at(hit_idx).unwrap();
        assert!(!p2_text.slice_text(w_start, w_end).is_empty());
        let quads = p2_text.selection_quads(thm_line.span_range.start, thm_line.span_range.end - 1);
        assert_eq!(quads.len(), 1);

        // Page 5 (/Rotate 90): Landscape line is wider than tall.
        let p5_text = extract_page_text(&handle, PageIndex(4)).unwrap();
        assert!(
            p5_text
                .full_text
                .contains("Rotated Landscape Appendix (/Rotate 90)"),
            "got rotated text:\n{}",
            p5_text.full_text
        );
        let first_line = &p5_text.lines[0];
        assert!(
            first_line.bounds.width() > first_line.bounds.height() * 4.0,
            "expected upright horizontal landscape line, got {:?}",
            first_line.bounds
        );
    }

    #[test]
    fn internal_links_resolve_correctly() {
        let bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(bytes.clone()).unwrap();
        let links = extract_all_links(&bytes, &handle.page_transforms());

        let p2_links = links.get(&PageIndex(1)).expect("page 2 should have link");
        assert_eq!(p2_links.len(), 1);
        assert!(matches!(
            p2_links[0].target,
            LinkTarget::InternalPage {
                page: PageIndex(3),
                ..
            }
        ));

        let p4_links = links.get(&PageIndex(3)).expect("page 4 should have back-link");
        assert_eq!(p4_links.len(), 1);
        assert!(matches!(
            p4_links[0].target,
            LinkTarget::InternalPage {
                page: PageIndex(1),
                ..
            }
        ));
    }
}
