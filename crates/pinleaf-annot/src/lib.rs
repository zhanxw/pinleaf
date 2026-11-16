pub mod appearance;
pub mod reader;
pub mod writer;

pub use reader::{AnnotError, load_annotations};
pub use writer::{SaveResult, save_annotations_atomic, save_annotations_to_bytes};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pinleaf_core::{
        Annotation, AnnotationId, AnnotationKind, AnnotationStore, NormQuad, NormRect, PageIndex,
        RgbaColor,
    };
    use pinleaf_engine::demo_fixture::build_demo_pdf_bytes;
    use pinleaf_engine::{PdfDocumentHandle, extract_page_text, render_page_bgra};

    use crate::{load_annotations, save_annotations_atomic, save_annotations_to_bytes};

    #[test]
    fn round_trip_annotations_including_rotated_page_and_unicode_note() {
        let initial_bytes = Arc::new(build_demo_pdf_bytes());
        let handle = PdfDocumentHandle::from_bytes(initial_bytes.clone()).unwrap();
        let transforms = handle.page_transforms();

        // 1. Load pre-existing Page 1 highlight.
        let loaded = load_annotations(&initial_bytes, &transforms).unwrap();
        assert_eq!(loaded.len(), 1);
        let original = &loaded[0];
        assert_eq!(original.page, PageIndex(0));
        assert_eq!(original.kind, AnnotationKind::Highlight);
        assert_eq!(
            original.id.to_pdf_nm(),
            "pinleaf:11111111-1111-4111-8111-111111111111"
        );
        assert!(original.pdf_object_id.is_some());
        assert_eq!(
            original.note_contents.as_deref(),
            Some("Pre-existing PDF highlight saved inside the demo document.")
        );

        let orig_id = original.id;
        let mut store = AnnotationStore::from_loaded(loaded);

        // 2. Delete original highlight.
        assert!(store.delete(orig_id));

        // 3. Add new Highlight with Unicode note on Page 2.
        let p2_rect = NormRect::new(54.0, 100.0, 320.0, 114.0);
        let p2_hl_id = AnnotationId::new();
        store.insert(Annotation {
            id: p2_hl_id,
            pdf_object_id: None,
            page: PageIndex(1),
            kind: AnnotationKind::Highlight,
            quads: vec![NormQuad::from_rect(p2_rect)],
            bounds: p2_rect,
            color: RgbaColor::GREEN,
            selected_text: Some("Theorem 1".into()),
            note_contents: Some("Check Lemma 2 — α + β".into()),
            modified_at: "D:20261116103500Z".into(),
        });

        // 4. Add Underline on Page 1, AreaNote on Page 3, and Highlight on rotated Page 5.
        let p1_ul_rect = NormRect::new(54.0, 174.0, 190.0, 188.0);
        let p1_ul_id = AnnotationId::new();
        store.insert(Annotation {
            id: p1_ul_id,
            pdf_object_id: None,
            page: PageIndex(0),
            kind: AnnotationKind::Underline,
            quads: vec![NormQuad::from_rect(p1_ul_rect)],
            bounds: p1_ul_rect,
            color: RgbaColor::BLUE,
            selected_text: Some("1. Introduction".into()),
            note_contents: None,
            modified_at: "D:20261116103501Z".into(),
        });

        let p3_area_rect = NormRect::new(70.0, 180.0, 210.0, 262.0);
        let p3_area_id = AnnotationId::new();
        store.insert(Annotation {
            id: p3_area_id,
            pdf_object_id: None,
            page: PageIndex(2),
            kind: AnnotationKind::AreaNote,
            quads: vec![],
            bounds: p3_area_rect,
            color: RgbaColor::ORANGE,
            selected_text: None,
            note_contents: Some("Diagram box #1".into()),
            modified_at: "D:20261116103502Z".into(),
        });

        let p5_rect = NormRect::new(60.0, 72.0, 350.0, 90.0);
        let p5_id = AnnotationId::new();
        store.insert(Annotation {
            id: p5_id,
            pdf_object_id: None,
            page: PageIndex(4),
            kind: AnnotationKind::Highlight,
            quads: vec![NormQuad::from_rect(p5_rect)],
            bounds: p5_rect,
            color: RgbaColor::PINK,
            selected_text: Some("Rotated Landscape Appendix".into()),
            note_contents: Some("Landscape check".into()),
            modified_at: "D:20261116103503Z".into(),
        });

        let diff = store.pending_diff();
        let saved = save_annotations_to_bytes(&initial_bytes, &diff, &transforms).unwrap();
        assert_eq!(saved.assigned_ids.len(), 4);

        // 5. Reload saved bytes in BOTH pdf-annot AND hayro (pdf-engine)!
        let saved_bytes = Arc::new(saved.bytes);
        let reloaded_handle =
            PdfDocumentHandle::from_bytes(saved_bytes.clone()).expect("hayro reads saved PDF");
        for p in 0..5 {
            let frame = render_page_bgra(&reloaded_handle, PageIndex(p), 1.0).unwrap();
            assert!(frame.width > 100);
        }
        let p5_text = extract_page_text(&reloaded_handle, PageIndex(4)).unwrap();
        assert!(p5_text.full_text.contains("Rotated Landscape Appendix"));

        let reloaded_annots = load_annotations(&saved_bytes, &transforms).unwrap();
        assert_eq!(reloaded_annots.len(), 4);
        assert!(reloaded_annots.iter().all(|a| a.id != orig_id));

        let reloaded_p2 = reloaded_annots.iter().find(|a| a.id == p2_hl_id).unwrap();
        assert_eq!(
            reloaded_p2.note_contents.as_deref(),
            Some("Check Lemma 2 — α + β")
        );

        let reloaded_p5 = reloaded_annots.iter().find(|a| a.id == p5_id).unwrap();
        assert!((reloaded_p5.bounds.min_x - p5_rect.min_x).abs() < 0.5);
        assert!((reloaded_p5.bounds.min_y - p5_rect.min_y).abs() < 0.5);
        assert!((reloaded_p5.bounds.max_x - p5_rect.max_x).abs() < 0.5);
        assert!((reloaded_p5.bounds.max_y - p5_rect.max_y).abs() < 0.5);

        let reloaded_p3 = reloaded_annots.iter().find(|a| a.id == p3_area_id).unwrap();
        assert_eq!(reloaded_p3.kind, AnnotationKind::AreaNote);
        assert_eq!(reloaded_p3.note_contents.as_deref(), Some("Diagram box #1"));

        // 6. Verify atomic save to disk + subsequent edit cycle.
        store.mark_saved(&saved.assigned_ids);
        assert!(!store.is_dirty());

        let tmp_path = std::env::temp_dir().join(format!(
            "pinleaf_annot_test_{}.pdf",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&tmp_path, &*saved_bytes).unwrap();

        store.update_note(p2_hl_id, Some("Updated note ✓".into()), "D:20261116103600Z".into());
        let diff2 = store.pending_diff();
        let saved2 = save_annotations_atomic(&saved_bytes, &tmp_path, &diff2, &transforms).unwrap();
        let disk_bytes = std::fs::read(&tmp_path).unwrap();
        assert_eq!(disk_bytes, saved2.bytes);

        let reloaded2 = load_annotations(&disk_bytes, &transforms).unwrap();
        let updated = reloaded2.iter().find(|a| a.id == p2_hl_id).unwrap();
        assert_eq!(updated.note_contents.as_deref(), Some("Updated note ✓"));

        let _ = std::fs::remove_file(&tmp_path);
    }
}
