use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::Path;

use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId, StringFormat, dictionary};
use pinleaf_core::{Annotation, AnnotationDiff, AnnotationId, AnnotationKind, PageIndex, PageTransform};

use crate::appearance::build_appearance_stream;
use crate::reader::AnnotError;

#[derive(Clone, Debug)]
pub struct SaveResult {
    pub bytes: Vec<u8>,
    pub assigned_ids: Vec<(AnnotationId, (u32, u16))>,
}

pub fn save_annotations_to_bytes(
    original_bytes: &[u8],
    diff: &AnnotationDiff,
    transforms: &[PageTransform],
) -> Result<SaveResult, AnnotError> {
    if diff.is_empty() {
        return Ok(SaveResult {
            bytes: original_bytes.to_vec(),
            assigned_ids: Vec::new(),
        });
    }

    match apply_incremental(original_bytes, diff, transforms) {
        Ok(res) => Ok(res),
        Err(_) => apply_full_rewrite(original_bytes, diff, transforms),
    }
}

pub fn save_annotations_atomic(
    original_bytes: &[u8],
    target_path: &Path,
    diff: &AnnotationDiff,
    transforms: &[PageTransform],
) -> Result<SaveResult, AnnotError> {
    let result = save_annotations_to_bytes(original_bytes, diff, transforms)?;
    let parent = target_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf");
    let tmp_path = parent.join(format!(".{file_name}.saving.tmp"));

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(&result.bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, target_path)?;
    Ok(result)
}

fn apply_incremental(
    original_bytes: &[u8],
    diff: &AnnotationDiff,
    transforms: &[PageTransform],
) -> Result<SaveResult, AnnotError> {
    let mut inc = IncrementalDocument::load_from(Cursor::new(original_bytes))?;
    let page_map = page_id_map(inc.get_prev_documents());
    let mut assigned_ids = Vec::new();

    // 1. Apply deletions first.
    for &(page_idx, obj_id) in &diff.deleted_pdf_objs {
        if let Some(&page_id) = page_map.get(&page_idx) {
            remove_annot_ref_incremental(&mut inc, page_id, obj_id)?;
        }
    }

    // 2. Apply upserts (updates or inserts).
    for ann in &diff.upserted {
        let &page_id = page_map
            .get(&ann.page)
            .ok_or(AnnotError::InvalidPage(ann.page.0))?;
        let &transform = transforms
            .get(ann.page.as_usize())
            .ok_or(AnnotError::InvalidPage(ann.page.0))?;

        let (dict, ap_stream) = build_annot_dict_and_ap(ann, transform);
        let ap_id = inc.new_document.add_object(ap_stream);
        let mut dict = dict;
        dict.set("AP", dictionary! { "N" => Object::Reference(ap_id) });

        if let Some(existing_id) = ann.pdf_object_id {
            inc.new_document
                .set_object(existing_id, Object::Dictionary(dict));
            assigned_ids.push((ann.id, existing_id));
        } else {
            let new_id = inc.new_document.add_object(dict);
            append_annot_ref_incremental(&mut inc, page_id, new_id)?;
            assigned_ids.push((ann.id, new_id));
        }
    }

    let mut out = Vec::new();
    inc.save_to(&mut out)?;
    Ok(SaveResult {
        bytes: out,
        assigned_ids,
    })
}

fn apply_full_rewrite(
    original_bytes: &[u8],
    diff: &AnnotationDiff,
    transforms: &[PageTransform],
) -> Result<SaveResult, AnnotError> {
    let mut doc = Document::load_mem(original_bytes)?;
    let page_map = page_id_map(&doc);
    let mut assigned_ids = Vec::new();

    for &(page_idx, obj_id) in &diff.deleted_pdf_objs {
        if let Some(&page_id) = page_map.get(&page_idx) {
            remove_annot_ref_doc(&mut doc, page_id, obj_id)?;
        }
    }

    for ann in &diff.upserted {
        let &page_id = page_map
            .get(&ann.page)
            .ok_or(AnnotError::InvalidPage(ann.page.0))?;
        let &transform = transforms
            .get(ann.page.as_usize())
            .ok_or(AnnotError::InvalidPage(ann.page.0))?;

        let (mut dict, ap_stream) = build_annot_dict_and_ap(ann, transform);
        let ap_id = doc.add_object(ap_stream);
        dict.set("AP", dictionary! { "N" => Object::Reference(ap_id) });

        if let Some(existing_id) = ann.pdf_object_id {
            doc.set_object(existing_id, Object::Dictionary(dict));
            assigned_ids.push((ann.id, existing_id));
        } else {
            let new_id = doc.add_object(dict);
            append_annot_ref_doc(&mut doc, page_id, new_id)?;
            assigned_ids.push((ann.id, new_id));
        }
    }

    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(SaveResult {
        bytes: out,
        assigned_ids,
    })
}

fn page_id_map(doc: &Document) -> HashMap<PageIndex, ObjectId> {
    doc.get_pages()
        .into_iter()
        .enumerate()
        .map(|(idx, (_, id))| (PageIndex(idx as u32), id))
        .collect()
}

fn remove_annot_ref_incremental(
    inc: &mut IncrementalDocument,
    page_id: ObjectId,
    target_annot_id: ObjectId,
) -> Result<(), AnnotError> {
    inc.opt_clone_object_to_new_document(page_id)?;
    let annots_ref = {
        let page_dict = inc.new_document.get_dictionary(page_id)?;
        match page_dict.get(b"Annots") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };

    if let Some(arr_id) = annots_ref {
        inc.opt_clone_object_to_new_document(arr_id)?;
        if let Ok(arr) = inc.new_document.get_object_mut(arr_id).and_then(Object::as_array_mut) {
            arr.retain(|item| item.as_reference().ok() != Some(target_annot_id));
        }
    } else if let Ok(page_dict) = inc
        .new_document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        && let Ok(Object::Array(arr)) = page_dict.get_mut(b"Annots")
    {
        arr.retain(|item| item.as_reference().ok() != Some(target_annot_id));
    }
    Ok(())
}

fn append_annot_ref_incremental(
    inc: &mut IncrementalDocument,
    page_id: ObjectId,
    new_annot_id: ObjectId,
) -> Result<(), AnnotError> {
    inc.opt_clone_object_to_new_document(page_id)?;
    let annots_ref = {
        let page_dict = inc.new_document.get_dictionary(page_id)?;
        match page_dict.get(b"Annots") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };

    if let Some(arr_id) = annots_ref {
        inc.opt_clone_object_to_new_document(arr_id)?;
        let arr = inc
            .new_document
            .get_object_mut(arr_id)
            .and_then(Object::as_array_mut)?;
        arr.push(Object::Reference(new_annot_id));
    } else {
        let page_dict = inc
            .new_document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)?;
        if let Ok(Object::Array(arr)) = page_dict.get_mut(b"Annots") {
            arr.push(Object::Reference(new_annot_id));
        } else {
            page_dict.set("Annots", Object::Array(vec![Object::Reference(new_annot_id)]));
        }
    }
    Ok(())
}

fn remove_annot_ref_doc(
    doc: &mut Document,
    page_id: ObjectId,
    target_annot_id: ObjectId,
) -> Result<(), AnnotError> {
    let annots_ref = {
        let page_dict = doc.get_dictionary(page_id)?;
        match page_dict.get(b"Annots") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };
    if let Some(arr_id) = annots_ref {
        if let Ok(arr) = doc.get_object_mut(arr_id).and_then(Object::as_array_mut) {
            arr.retain(|item| item.as_reference().ok() != Some(target_annot_id));
        }
    } else if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(Object::as_dict_mut)
        && let Ok(Object::Array(arr)) = page_dict.get_mut(b"Annots")
    {
        arr.retain(|item| item.as_reference().ok() != Some(target_annot_id));
    }
    Ok(())
}

fn append_annot_ref_doc(
    doc: &mut Document,
    page_id: ObjectId,
    new_annot_id: ObjectId,
) -> Result<(), AnnotError> {
    let annots_ref = {
        let page_dict = doc.get_dictionary(page_id)?;
        match page_dict.get(b"Annots") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };
    if let Some(arr_id) = annots_ref {
        let arr = doc.get_object_mut(arr_id).and_then(Object::as_array_mut)?;
        arr.push(Object::Reference(new_annot_id));
    } else {
        let page_dict = doc
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)?;
        if let Ok(Object::Array(arr)) = page_dict.get_mut(b"Annots") {
            arr.push(Object::Reference(new_annot_id));
        } else {
            page_dict.set("Annots", Object::Array(vec![Object::Reference(new_annot_id)]));
        }
    }
    Ok(())
}

fn build_annot_dict_and_ap(
    ann: &Annotation,
    transform: PageTransform,
) -> (Dictionary, lopdf::Stream) {
    let pdf_rect = transform.norm_rect_to_pdf(ann.bounds);
    let subtype = match ann.kind {
        AnnotationKind::Highlight => "Highlight",
        AnnotationKind::Underline => "Underline",
        AnnotationKind::StrikeOut => "StrikeOut",
        AnnotationKind::AreaNote => "Square",
        AnnotationKind::StickyNote => "Text",
    };

    let mut dict = dictionary! {
        "Type" => "Annot",
        "Subtype" => subtype,
        "NM" => Object::string_literal(ann.id.to_pdf_nm()),
        "Rect" => vec![
            pdf_rect[0].into(),
            pdf_rect[1].into(),
            pdf_rect[2].into(),
            pdf_rect[3].into(),
        ],
        "C" => vec![
            ann.color.r.into(),
            ann.color.g.into(),
            ann.color.b.into(),
        ],
        "CA" => ann.color.a,
        "F" => 4, // Print flag per ISO-32000 best practice
    };

    if !ann.modified_at.is_empty() {
        dict.set("M", Object::string_literal(ann.modified_at.as_str()));
    }

    if let Some(note) = &ann.note_contents {
        dict.set("Contents", encode_pdf_string(note));
    }

    if matches!(
        ann.kind,
        AnnotationKind::Highlight | AnnotationKind::Underline | AnnotationKind::StrikeOut
    ) {
        let mut quad_floats = Vec::with_capacity(ann.quads.len() * 8);
        for quad in &ann.quads {
            // Write in Adobe [TL, TR, BL, BR] order in PDF User Space.
            for &corner in &quad.corners {
                let pt = transform.norm_to_pdf(corner);
                quad_floats.push(Object::Real(pt.x));
                quad_floats.push(Object::Real(pt.y));
            }
        }
        if !quad_floats.is_empty() {
            dict.set("QuadPoints", Object::Array(quad_floats));
        }
    }

    if ann.kind == AnnotationKind::AreaNote {
        dict.set(
            "IC",
            vec![ann.color.r.into(), ann.color.g.into(), ann.color.b.into()],
        );
    }

    let ap_stream = build_appearance_stream(ann, pdf_rect, transform);
    (dict, ap_stream)
}

fn encode_pdf_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::string_literal(s)
    } else {
        let mut bytes = Vec::with_capacity(2 + s.len() * 2);
        bytes.extend_from_slice(&[0xFE, 0xFF]);
        for unit in s.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Hexadecimal)
    }
}
