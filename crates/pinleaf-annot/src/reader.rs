use lopdf::{Dictionary, Document, Object, ObjectId};
use pinleaf_core::{
    Annotation, AnnotationId, AnnotationKind, NormPoint, NormQuad, NormRect, PageIndex,
    PageTransform, PdfPoint, RgbaColor, normalize_category_char,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnnotError {
    #[error("failed to load PDF for annotation I/O: {0}")]
    Lopdf(#[from] lopdf::Error),
    #[error("I/O error while saving PDF: {0}")]
    Io(#[from] std::io::Error),
    #[error("page index {0} out of range")]
    InvalidPage(u32),
}

pub fn load_annotations(
    pdf_bytes: &[u8],
    transforms: &[PageTransform],
) -> Result<Vec<Annotation>, AnnotError> {
    let doc = Document::load_mem(pdf_bytes)?;
    let pages = doc.get_pages();
    let mut out = Vec::new();

    for (idx, (_, &page_id)) in pages.iter().enumerate() {
        let Some(&transform) = transforms.get(idx) else {
            continue;
        };
        let page_idx = PageIndex(idx as u32);
        let Ok(page_dict) = doc.get_dictionary(page_id) else {
            continue;
        };
        let Ok(annots_obj) = page_dict.get(b"Annots") else {
            continue;
        };
        let Some(annots_arr) = resolve_array(&doc, annots_obj) else {
            continue;
        };

        for item in annots_arr {
            let pdf_object_id = item.as_reference().ok();
            let Some(dict) = resolve_dict(&doc, item) else {
                continue;
            };
            if let Some(ann) = parse_annotation_dict(&doc, dict, pdf_object_id, page_idx, transform) {
                out.push(ann);
            }
        }
    }

    Ok(out)
}

fn parse_annotation_dict(
    doc: &Document,
    dict: &Dictionary,
    pdf_object_id: Option<ObjectId>,
    page: PageIndex,
    transform: PageTransform,
) -> Option<Annotation> {
    let subtype = dict.get(b"Subtype").ok()?.as_name().ok()?;
    let kind = match subtype {
        b"Highlight" => AnnotationKind::Highlight,
        b"Underline" => AnnotationKind::Underline,
        b"StrikeOut" => AnnotationKind::StrikeOut,
        b"Square" => AnnotationKind::AreaNote,
        b"Text" => AnnotationKind::StickyNote,
        _ => return None,
    };

    let rect_arr = parse_f32_array(doc, dict.get(b"Rect").ok()?)?;
    if rect_arr.len() < 4 {
        return None;
    }
    let pdf_rect = [rect_arr[0], rect_arr[1], rect_arr[2], rect_arr[3]];
    let norm_rect = transform.pdf_rect_to_norm(pdf_rect);

    let mut quads = Vec::new();
    if let Ok(qp_obj) = dict.get(b"QuadPoints")
        && let Some(qp_arr) = parse_f32_array(doc, qp_obj)
    {
        for chunk in qp_arr.chunks_exact(8) {
            let pts = [
                transform.pdf_to_norm(PdfPoint::new(chunk[0], chunk[1])),
                transform.pdf_to_norm(PdfPoint::new(chunk[2], chunk[3])),
                transform.pdf_to_norm(PdfPoint::new(chunk[4], chunk[5])),
                transform.pdf_to_norm(PdfPoint::new(chunk[6], chunk[7])),
            ];
            quads.push(NormQuad::from_unordered_points(pts));
        }
    } else if matches!(
        kind,
        AnnotationKind::Highlight | AnnotationKind::Underline | AnnotationKind::StrikeOut
    ) {
        quads.push(NormQuad::from_rect(norm_rect));
    }

    let bounds = if quads.is_empty() {
        norm_rect
    } else {
        quads
            .iter()
            .map(NormQuad::bounding_rect)
            .reduce(NormRect::union)
            .unwrap_or(norm_rect)
    };

    let alpha = dict
        .get(b"CA")
        .ok()
        .and_then(|o| object_as_f32(deref(doc, o)))
        .unwrap_or(0.36);

    let color = dict
        .get(b"C")
        .ok()
        .and_then(|o| parse_f32_array(doc, o))
        .map(|c| match c.as_slice() {
            [r, g, b, ..] => RgbaColor::new(*r, *g, *b, alpha),
            [gray] => RgbaColor::new(*gray, *gray, *gray, alpha),
            _ => RgbaColor::YELLOW,
        })
        .unwrap_or_else(|| RgbaColor::new(1.0, 0.90, 0.20, alpha));

    let id = dict
        .get(b"NM")
        .ok()
        .and_then(|o| decode_pdf_string(deref(doc, o)))
        .and_then(|s| AnnotationId::from_pdf_nm(&s))
        .unwrap_or_default();

    let note_contents = dict
        .get(b"Contents")
        .ok()
        .and_then(|o| decode_pdf_string(deref(doc, o)))
        .filter(|s| !s.trim().is_empty());

    let modified_at = dict
        .get(b"M")
        .ok()
        .and_then(|o| decode_pdf_string(deref(doc, o)))
        .unwrap_or_default();

    let category = dict
        .get(b"Subj")
        .ok()
        .and_then(|o| decode_pdf_string(deref(doc, o)))
        .and_then(|s| s.strip_prefix("pinleaf-cat:").and_then(|c| c.chars().next()))
        .and_then(normalize_category_char);

    // Preserve a readable point-order fallback if a quad was empty.
    let _ = NormPoint::new(0.0, 0.0);

    Some(Annotation {
        id,
        pdf_object_id,
        page,
        kind,
        quads,
        bounds,
        color,
        category,
        selected_text: None,
        note_contents,
        modified_at,
    })
}

pub(crate) fn decode_pdf_string(obj: &Object) -> Option<String> {
    let bytes = obj.as_str().ok()?;
    if bytes.starts_with(&[0xFE, 0xFF]) && bytes.len() >= 2 {
        let mut units = Vec::with_capacity((bytes.len() - 2) / 2);
        for pair in bytes[2..].chunks_exact(2) {
            units.push(u16::from_be_bytes([pair[0], pair[1]]));
        }
        return Some(String::from_utf16_lossy(&units));
    }
    if let Ok(utf8) = std::str::from_utf8(bytes) {
        return Some(utf8.to_owned());
    }
    Some(bytes.iter().map(|&b| b as char).collect())
}

fn deref<'a>(doc: &'a Document, mut obj: &'a Object) -> &'a Object {
    for _ in 0..8 {
        if let Object::Reference(id) = obj {
            if let Ok(next) = doc.get_object(*id) {
                obj = next;
                continue;
            }
        }
        break;
    }
    obj
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    deref(doc, obj).as_dict().ok()
}

fn resolve_array<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Vec<Object>> {
    deref(doc, obj).as_array().ok()
}

fn parse_f32_array(doc: &Document, obj: &Object) -> Option<Vec<f32>> {
    let arr = resolve_array(doc, obj)?;
    arr.iter()
        .map(|item| object_as_f32(deref(doc, item)))
        .collect()
}

fn object_as_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Real(v) => Some(*v),
        Object::Integer(v) => Some(*v as f32),
        _ => None,
    }
}
