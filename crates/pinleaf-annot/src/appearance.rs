use lopdf::content::{Content, Operation};
use lopdf::{Stream, dictionary};
use pinleaf_core::{Annotation, AnnotationKind, PageTransform};

/// Builds a PDF Form XObject `/AP << /N ... >>` appearance stream with Multiply blending
/// so external viewers (Preview, Acrobat, Chrome) render highlights transparently over text.
pub fn build_appearance_stream(
    ann: &Annotation,
    pdf_rect: [f32; 4],
    transform: PageTransform,
) -> Stream {
    let [rx0, ry0, rx1, ry1] = pdf_rect;
    let w = (rx1 - rx0).max(1.0);
    let h = (ry1 - ry0).max(1.0);
    let r = ann.color.r.clamp(0.0, 1.0);
    let g = ann.color.g.clamp(0.0, 1.0);
    let b = ann.color.b.clamp(0.0, 1.0);

    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new("gs", vec!["GS0".into()]),
        Operation::new("rg", vec![r.into(), g.into(), b.into()]),
        Operation::new("RG", vec![r.into(), g.into(), b.into()]),
    ];

    match ann.kind {
        AnnotationKind::Highlight => {
            if ann.quads.is_empty() {
                ops.push(Operation::new(
                    "re",
                    vec![0.0.into(), 0.0.into(), w.into(), h.into()],
                ));
                ops.push(Operation::new("f", vec![]));
            } else {
                for quad in &ann.quads {
                    let p_tl = transform.norm_to_pdf(quad.corners[0]);
                    let p_tr = transform.norm_to_pdf(quad.corners[1]);
                    let p_bl = transform.norm_to_pdf(quad.corners[2]);
                    let p_br = transform.norm_to_pdf(quad.corners[3]);
                    ops.push(Operation::new(
                        "m",
                        vec![(p_tl.x - rx0).into(), (p_tl.y - ry0).into()],
                    ));
                    ops.push(Operation::new(
                        "l",
                        vec![(p_tr.x - rx0).into(), (p_tr.y - ry0).into()],
                    ));
                    ops.push(Operation::new(
                        "l",
                        vec![(p_br.x - rx0).into(), (p_br.y - ry0).into()],
                    ));
                    ops.push(Operation::new(
                        "l",
                        vec![(p_bl.x - rx0).into(), (p_bl.y - ry0).into()],
                    ));
                    ops.push(Operation::new("h", vec![]));
                    ops.push(Operation::new("f", vec![]));
                }
            }
        }
        AnnotationKind::Underline => {
            ops.push(Operation::new("w", vec![1.5.into()]));
            for quad in &ann.quads {
                let p_bl = transform.norm_to_pdf(quad.corners[2]);
                let p_br = transform.norm_to_pdf(quad.corners[3]);
                ops.push(Operation::new(
                    "m",
                    vec![(p_bl.x - rx0).into(), (p_bl.y - ry0).into()],
                ));
                ops.push(Operation::new(
                    "l",
                    vec![(p_br.x - rx0).into(), (p_br.y - ry0).into()],
                ));
                ops.push(Operation::new("S", vec![]));
            }
        }
        AnnotationKind::StrikeOut => {
            ops.push(Operation::new("w", vec![1.5.into()]));
            for quad in &ann.quads {
                let p_tl = transform.norm_to_pdf(quad.corners[0]);
                let p_tr = transform.norm_to_pdf(quad.corners[1]);
                let p_bl = transform.norm_to_pdf(quad.corners[2]);
                let p_br = transform.norm_to_pdf(quad.corners[3]);
                let mid_l = ((p_tl.x + p_bl.x) * 0.5 - rx0, (p_tl.y + p_bl.y) * 0.5 - ry0);
                let mid_r = ((p_tr.x + p_br.x) * 0.5 - rx0, (p_tr.y + p_br.y) * 0.5 - ry0);
                ops.push(Operation::new("m", vec![mid_l.0.into(), mid_l.1.into()]));
                ops.push(Operation::new("l", vec![mid_r.0.into(), mid_r.1.into()]));
                ops.push(Operation::new("S", vec![]));
            }
        }
        AnnotationKind::AreaNote => {
            ops.push(Operation::new("w", vec![1.6.into()]));
            ops.push(Operation::new(
                "re",
                vec![0.8.into(), 0.8.into(), (w - 1.6).max(1.0).into(), (h - 1.6).max(1.0).into()],
            ));
            ops.push(Operation::new("B", vec![]));
        }
        AnnotationKind::StickyNote => {
            ops.push(Operation::new(
                "re",
                vec![0.0.into(), 0.0.into(), w.into(), h.into()],
            ));
            ops.push(Operation::new("B", vec![]));
        }
    }

    ops.push(Operation::new("Q", vec![]));

    let bytes = Content { operations: ops }.encode().unwrap_or_default();
    let alpha_fill = match ann.kind {
        AnnotationKind::AreaNote => 0.18,
        _ => ann.color.a.clamp(0.15, 0.85),
    };

    Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.0.into(), 0.0.into(), w.into(), h.into()],
            "Resources" => dictionary! {
                "ExtGState" => dictionary! {
                    "GS0" => dictionary! {
                        "Type" => "ExtGState",
                        "BM" => "Multiply",
                        "ca" => alpha_fill,
                        "CA" => 0.9,
                    }
                }
            }
        },
        bytes,
    )
}
