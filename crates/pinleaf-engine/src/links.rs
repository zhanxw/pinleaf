use std::collections::HashMap;

use lopdf::{Dictionary, Document, Object, ObjectId};
use pinleaf_core::{NormPoint, NormRect, PageIndex, PageTransform, PdfPoint};

#[derive(Clone, Debug, PartialEq)]
pub enum LinkTarget {
    InternalPage {
        page: PageIndex,
        top_norm_y: Option<f32>,
    },
    ExternalUrl(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkRegion {
    pub rect: NormRect,
    pub target: LinkTarget,
}

/// Extracts all `/Subtype /Link` annotations across every page in Normalized Page Space.
pub fn extract_all_links(
    pdf_bytes: &[u8],
    transforms: &[PageTransform],
) -> HashMap<PageIndex, Vec<LinkRegion>> {
    let mut out = HashMap::new();
    let Ok(doc) = Document::load_mem(pdf_bytes) else {
        return out;
    };

    let named_dests = collect_named_dest_objects(&doc);

    let pages = doc.get_pages(); // BTreeMap<u32 (1-based), ObjectId>
    let mut obj_to_page_idx: HashMap<ObjectId, PageIndex> = HashMap::with_capacity(pages.len());
    let mut ordered_pages: Vec<(PageIndex, ObjectId)> = Vec::with_capacity(pages.len());

    for (idx, (_, &page_id)) in pages.iter().enumerate() {
        let page_idx = PageIndex(idx as u32);
        obj_to_page_idx.insert(page_id, page_idx);
        ordered_pages.push((page_idx, page_id));
    }

    for (page_idx, page_id) in ordered_pages {
        let Some(transform) = transforms.get(page_idx.as_usize()).copied() else {
            continue;
        };
        let Ok(page_dict) = doc.get_dictionary(page_id) else {
            continue;
        };
        let Ok(annots_obj) = page_dict.get(b"Annots") else {
            continue;
        };
        let Some(annots_arr) = resolve_array(&doc, annots_obj) else {
            continue;
        };

        let mut page_links = Vec::new();
        for item in annots_arr {
            let Some(dict) = resolve_dict(&doc, item) else {
                continue;
            };
            let is_link = dict
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name().ok())
                .is_some_and(|n| n == b"Link");
            if !is_link {
                continue;
            }
            let Some(pdf_rect) = parse_rect(dict.get(b"Rect").ok()) else {
                continue;
            };
            let rect = transform.pdf_rect_to_norm(pdf_rect);

            if let Some(target) =
                resolve_link_target(&doc, dict, &named_dests, &obj_to_page_idx, transforms)
            {
                page_links.push(LinkRegion { rect, target });
            }
        }

        if !page_links.is_empty() {
            out.insert(page_idx, page_links);
        }
    }

    out
}

fn collect_named_dest_objects(doc: &Document) -> HashMap<Vec<u8>, Object> {
    let mut map = HashMap::new();
    let Ok(catalog) = doc.catalog() else {
        return map;
    };

    if let Ok(dests_obj) = catalog.get(b"Dests")
        && let Some(dests_dict) = resolve_dict(doc, dests_obj)
    {
        for (k, v) in dests_dict.iter() {
            map.insert(k.clone(), v.clone());
        }
    }

    if let Ok(names_obj) = catalog.get(b"Names")
        && let Some(names_dict) = resolve_dict(doc, names_obj)
        && let Ok(dests_tree_obj) = names_dict.get(b"Dests")
        && let Some(dests_tree) = resolve_dict(doc, dests_tree_obj)
    {
        collect_name_tree(doc, dests_tree, 0, &mut map);
    }

    map
}

fn collect_name_tree(
    doc: &Document,
    node: &Dictionary,
    depth: usize,
    out: &mut HashMap<Vec<u8>, Object>,
) {
    if depth > 16 {
        return;
    }
    if let Ok(names_obj) = node.get(b"Names")
        && let Some(arr) = resolve_array(doc, names_obj)
    {
        for pair in arr.chunks_exact(2) {
            let key = deref(doc, &pair[0]);
            if let Ok(bytes) = key.as_str() {
                out.insert(bytes.to_vec(), pair[1].clone());
            } else if let Ok(bytes) = key.as_name() {
                out.insert(bytes.to_vec(), pair[1].clone());
            }
        }
    }
    if let Ok(kids_obj) = node.get(b"Kids")
        && let Some(kids) = resolve_array(doc, kids_obj)
    {
        for kid in kids {
            if let Some(kid_dict) = resolve_dict(doc, kid) {
                collect_name_tree(doc, kid_dict, depth + 1, out);
            }
        }
    }
}

fn resolve_link_target(
    doc: &Document,
    link_dict: &Dictionary,
    named_dests: &HashMap<Vec<u8>, Object>,
    obj_to_page: &HashMap<ObjectId, PageIndex>,
    transforms: &[PageTransform],
) -> Option<LinkTarget> {
    if let Ok(dest_obj) = link_dict.get(b"Dest")
        && let Some(target) =
            resolve_dest_object(doc, dest_obj, named_dests, obj_to_page, transforms)
    {
        return Some(target);
    }

    let action_obj = link_dict.get(b"A").ok()?;
    let action_dict = resolve_dict(doc, action_obj)?;
    let subtype = action_dict.get(b"S").ok()?.as_name().ok()?;
    if subtype == b"GoTo" {
        let dest_obj = action_dict.get(b"D").ok()?;
        return resolve_dest_object(doc, dest_obj, named_dests, obj_to_page, transforms);
    }
    if subtype == b"URI" {
        let uri_bytes = action_dict.get(b"URI").ok()?.as_str().ok()?;
        let uri = String::from_utf8_lossy(uri_bytes).into_owned();
        return Some(LinkTarget::ExternalUrl(uri));
    }

    None
}

fn resolve_dest_object(
    doc: &Document,
    dest_obj: &Object,
    named_dests: &HashMap<Vec<u8>, Object>,
    obj_to_page: &HashMap<ObjectId, PageIndex>,
    transforms: &[PageTransform],
) -> Option<LinkTarget> {
    resolve_dest_object_inner(doc, dest_obj, named_dests, obj_to_page, transforms, 0)
}

fn resolve_dest_object_inner(
    doc: &Document,
    dest_obj: &Object,
    named_dests: &HashMap<Vec<u8>, Object>,
    obj_to_page: &HashMap<ObjectId, PageIndex>,
    transforms: &[PageTransform],
    depth: usize,
) -> Option<LinkTarget> {
    if depth > 6 {
        return None;
    }
    let resolved = deref(doc, dest_obj);
    match resolved {
        Object::Array(arr) => parse_explicit_dest(arr, obj_to_page, transforms),
        Object::Dictionary(d) => {
            let inner = d.get(b"D").ok()?;
            resolve_dest_object_inner(doc, inner, named_dests, obj_to_page, transforms, depth + 1)
        }
        Object::String(name, _) | Object::Name(name) => {
            let target = named_dests.get(name)?;
            resolve_dest_object_inner(doc, target, named_dests, obj_to_page, transforms, depth + 1)
        }
        _ => None,
    }
}

fn parse_explicit_dest(
    arr: &[Object],
    obj_to_page: &HashMap<ObjectId, PageIndex>,
    transforms: &[PageTransform],
) -> Option<LinkTarget> {
    let page_ref = arr.first()?.as_reference().ok()?;
    let page_idx = *obj_to_page.get(&page_ref)?;
    let transform = transforms.get(page_idx.as_usize())?;

    let mut top_norm_y = None;
    if let Some(mode_obj) = arr.get(1)
        && let Ok(mode) = mode_obj.as_name()
    {
        if mode == b"XYZ" {
            if let Some(top_pdf) = arr.get(3).and_then(object_as_f32) {
                let left_pdf = arr.get(2).and_then(object_as_f32).unwrap_or(0.0);
                let norm: NormPoint = transform.pdf_to_norm(PdfPoint::new(left_pdf, top_pdf));
                top_norm_y = Some(norm.y.max(0.0));
            }
        } else if (mode == b"FitH" || mode == b"FitBH")
            && let Some(top_pdf) = arr.get(2).and_then(object_as_f32)
        {
            let norm = transform.pdf_to_norm(PdfPoint::new(transform.crop_box[0], top_pdf));
            top_norm_y = Some(norm.y.max(0.0));
        }
    }

    Some(LinkTarget::InternalPage {
        page: page_idx,
        top_norm_y,
    })
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

fn parse_rect(obj: Option<&Object>) -> Option<[f32; 4]> {
    let arr = obj?.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    Some([
        object_as_f32(&arr[0])?,
        object_as_f32(&arr[1])?,
        object_as_f32(&arr[2])?,
        object_as_f32(&arr[3])?,
    ])
}

fn object_as_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Real(v) => Some(*v),
        Object::Integer(v) => Some(*v as f32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lopdf::dictionary;

    use super::*;
    use crate::PdfDocumentHandle;

    #[test]
    fn named_destination_in_names_tree_resolves_target_page_and_y() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let p1_id = doc.new_object_id();
        let p2_id = doc.new_object_id();

        let p1_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![50.0.into(), 600.0.into(), 180.0.into(), 620.0.into()],
            "Dest" => Object::string_literal("sec.target"),
        });

        doc.objects.insert(
            p1_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
                "Annots" => vec![Object::Reference(p1_link)],
            }),
        );
        doc.objects.insert(
            p2_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
            }),
        );
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Count" => 2,
                "Kids" => vec![Object::Reference(p1_id), Object::Reference(p2_id)],
            }),
        );

        let dests_tree_id = doc.add_object(dictionary! {
            "Names" => vec![
                Object::string_literal("sec.target"),
                vec![Object::Reference(p2_id), "XYZ".into(), 0.0.into(), 700.0.into(), 0.0.into()].into(),
            ],
        });
        let names_id = doc.add_object(dictionary! {
            "Dests" => Object::Reference(dests_tree_id),
        });
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
            "Names" => Object::Reference(names_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let bytes = Arc::new(bytes);
        let handle = PdfDocumentHandle::from_bytes(bytes.clone()).unwrap();
        let links = extract_all_links(&bytes, &handle.page_transforms());

        let p1_links = links.get(&PageIndex(0)).expect("page 1 link present");
        assert_eq!(p1_links.len(), 1);
        match p1_links[0].target {
            LinkTarget::InternalPage { page, top_norm_y } => {
                assert_eq!(page, PageIndex(1));
                let y = top_norm_y.expect("XYZ top coordinate resolved");
                assert!((y - 92.0).abs() < 1.0, "expected 792 - 700 = 92, got {y}");
            }
            ref other => panic!("expected InternalPage, got {other:?}"),
        }
    }
}
