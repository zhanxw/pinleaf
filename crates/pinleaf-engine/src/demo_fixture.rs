use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, ObjectId, Stream, dictionary};

/// Builds an in-memory 5-page academic/technical PDF fixture featuring:
/// - Page 1: Title + Abstract + 2-column layout + pre-existing `/Highlight` annotation with `/Contents`
/// - Page 2: Theorem 1 + clickable internal `/Link` jumping to Proof on Page 4
/// - Page 3: Vector architecture diagram + Figure 1 caption (for Area/Square annotation testing)
/// - Page 4: Detailed Proof of Theorem 1 + back-link to Page 2
/// - Page 5: Rotated `/Rotate 90` landscape summary page
pub fn build_demo_pdf_bytes() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_regular = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let font_bold = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
        "Encoding" => "WinAnsiEncoding",
    });
    let font_mono = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
        "Encoding" => "WinAnsiEncoding",
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_regular),
            "F2" => Object::Reference(font_bold),
            "F3" => Object::Reference(font_mono),
        }
    });

    // Pre-allocate all 5 page ObjectIds so cross-page /Link destinations can reference Page 4 and Page 2.
    let p1_id = doc.new_object_id();
    let p2_id = doc.new_object_id();
    let p3_id = doc.new_object_id();
    let p4_id = doc.new_object_id();
    let p5_id = doc.new_object_id();

    // --- Page 1: 2-column paper + pre-existing Highlight annotation ---
    let mut p1_ops = Vec::new();
    push_text(&mut p1_ops, "F2", 18.0, 54.0, 735.0, "Freeze-and-Branch Multi-View Reading Systems");
    push_text(&mut p1_ops, "F1", 10.5, 54.0, 714.0, "Pinleaf — Pure Rust PDF Engine & GPU Multi-View Reader");
    push_rule(&mut p1_ops, 54.0, 704.0, 558.0, 704.0);
    push_text(&mut p1_ops, "F2", 11.5, 54.0, 682.0, "Abstract - Deep Technical Reading");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 665.0, "Reading proofs and system diagrams often requires pinning earlier definitions");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 651.0, "while continuing forward through subsequent sections, lemmas, and appendices.");
    // Left column (x = 54 .. 285)
    push_text(&mut p1_ops, "F2", 11.0, 54.0, 616.0, "1. Introduction");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 598.0, "Single-viewport readers lose context");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 583.0, "whenever the reader follows a figure");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 568.0, "reference or theorem citation.");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 546.0, "Freezing the active viewport locks its");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 531.0, "reading anchor while opening a live");
    push_text(&mut p1_ops, "F1", 10.0, 54.0, 516.0, "child panel at the exact same scroll.");
    push_text(&mut p1_ops, "F1", 9.5, 54.0, 496.0, "See Figure 1, Theorem 1, and [1].");
    // Right column (x = 320 .. 558)
    push_text(&mut p1_ops, "F2", 11.0, 320.0, 616.0, "2. Core Invariants");
    push_text(&mut p1_ops, "F1", 10.0, 320.0, 598.0, "Every open panel and OS window shares");
    push_text(&mut p1_ops, "F1", 10.0, 320.0, 583.0, "one immutable PDF byte buffer and one");
    push_text(&mut p1_ops, "F1", 10.0, 320.0, 568.0, "live-synchronized annotation store.");
    push_text(&mut p1_ops, "F1", 10.0, 320.0, 546.0, "Text selection remains active inside");
    push_text(&mut p1_ops, "F1", 10.0, 320.0, 531.0, "frozen panels so pinned quotes can");
    push_text(&mut p1_ops, "F1", 10.0, 320.0, 516.0, "still be highlighted and copied.");
    push_text(&mut p1_ops, "F1", 9.5, 54.0, 48.0, "Page 1 of 5 - Try Cmd+D to Freeze & Split Right, or F to toggle Freeze");

    let p1_content = add_content_stream(&mut doc, p1_ops);
    let p1_annot = add_preexisting_highlight(
        &mut doc,
        [52.0, 661.0, 405.0, 676.0],
        "Pre-existing PDF highlight saved inside the demo document.",
    );
    doc.objects.insert(
        p1_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
            "Resources" => Object::Reference(resources_id),
            "Contents" => Object::Reference(p1_content),
            "Annots" => vec![Object::Reference(p1_annot)],
        }),
    );

    // --- Page 2: Theorem 1 + Clickable Link to Page 4 ---
    let mut p2_ops = Vec::new();
    push_text(&mut p2_ops, "F2", 15.0, 54.0, 730.0, "Section 3: Normalized Coordinate Invariants");
    push_text(&mut p2_ops, "F2", 12.0, 54.0, 690.0, "Theorem 1 (Multi-View Reading Invariant)");
    push_text(&mut p2_ops, "F1", 10.5, 54.0, 670.0, "For any page transform T and any set of open viewports V1..Vk operating");
    push_text(&mut p2_ops, "F1", 10.5, 54.0, 654.0, "at arbitrary zoom scales s_i, storing annotations in Normalized Page Space");
    push_text(&mut p2_ops, "F1", 10.5, 54.0, 638.0, "ensures zero drift across pane splits, window tear-offs, and PDF saves.");
    push_box(&mut p2_ops, 54.0, 580.0, 260.0, 26.0, [0.93, 0.96, 1.0], [0.15, 0.40, 0.85]);
    push_text(&mut p2_ops, "F2", 10.5, 64.0, 589.0, "-> Jump to Proof of Theorem 1 (Page 4)");
    push_text(&mut p2_ops, "F1", 10.0, 54.0, 540.0, "Tip: Freeze this page with F, then click the blue link above!");
    push_text(&mut p2_ops, "F1", 10.0, 54.0, 524.0, "The frozen panel stays locked on Theorem 1 while a child pane jumps to Page 4.");
    push_text(&mut p2_ops, "F1", 9.5, 54.0, 48.0, "Page 2 of 5");

    let p2_content = add_content_stream(&mut doc, p2_ops);
    let p2_link = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => vec![54.0.into(), 580.0.into(), 314.0.into(), 606.0.into()],
        "Border" => vec![0.into(), 0.into(), 0.into()],
        "A" => dictionary! {
            "S" => "GoTo",
            "D" => vec![Object::Reference(p4_id), "XYZ".into(), 0.0.into(), 760.0.into(), 0.0.into()],
        }
    });
    doc.objects.insert(
        p2_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
            "Resources" => Object::Reference(resources_id),
            "Contents" => Object::Reference(p2_content),
            "Annots" => vec![Object::Reference(p2_link)],
        }),
    );

    // --- Page 3: Vector Diagram for Alt+Drag Area Annotation ---
    let mut p3_ops = Vec::new();
    push_text(&mut p3_ops, "F2", 15.0, 54.0, 730.0, "Figure 1: Freeze & Branch Architecture");
    push_text(&mut p3_ops, "F1", 10.0, 54.0, 708.0, "Hold Alt and drag a rectangle over the diagram below to create an Area Note.");
    push_box(&mut p3_ops, 70.0, 530.0, 140.0, 80.0, [0.98, 0.94, 0.82], [0.80, 0.55, 0.10]);
    push_text(&mut p3_ops, "F2", 11.0, 88.0, 578.0, "Viewport #1");
    push_text(&mut p3_ops, "F1", 9.5, 88.0, 560.0, "p.2 [FROZEN]");
    push_box(&mut p3_ops, 240.0, 530.0, 140.0, 80.0, [0.98, 0.94, 0.82], [0.80, 0.55, 0.10]);
    push_text(&mut p3_ops, "F2", 11.0, 258.0, 578.0, "Viewport #2");
    push_text(&mut p3_ops, "F1", 9.5, 258.0, 560.0, "p.3 [FROZEN]");
    push_box(&mut p3_ops, 410.0, 530.0, 140.0, 80.0, [0.86, 0.95, 0.90], [0.15, 0.55, 0.30]);
    push_text(&mut p3_ops, "F2", 11.0, 428.0, 578.0, "Viewport #3");
    push_text(&mut p3_ops, "F1", 9.5, 428.0, 560.0, "p.4 [ACTIVE]");
    push_rule(&mut p3_ops, 210.0, 570.0, 240.0, 570.0);
    push_rule(&mut p3_ops, 380.0, 570.0, 410.0, 570.0);
    push_text(&mut p3_ops, "F1", 10.0, 70.0, 495.0, "All viewports observe a single DocumentSession and AnnotationStore.");
    push_text(&mut p3_ops, "F1", 9.5, 54.0, 48.0, "Page 3 of 5");

    let p3_content = add_content_stream(&mut doc, p3_ops);
    doc.objects.insert(
        p3_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
            "Resources" => Object::Reference(resources_id),
            "Contents" => Object::Reference(p3_content),
        }),
    );

    // --- Page 4: Proof of Theorem 1 + Back-Link to Page 2 ---
    let mut p4_ops = Vec::new();
    push_text(&mut p4_ops, "F2", 15.0, 54.0, 730.0, "Proof of Theorem 1");
    push_text(&mut p4_ops, "F1", 10.5, 54.0, 696.0, "Let T_p denote the crop-box and rotation bijection mapping PDF User Space");
    push_text(&mut p4_ops, "F1", 10.5, 54.0, 680.0, "into visual Normalized Page Space [0, W] x [0, H].");
    push_text(&mut p4_ops, "F3", 9.5, 64.0, 650.0, "norm_to_pdf(pdf_to_norm(x, y)) == (x, y)  for R in {0, 90, 180, 270}");
    push_text(&mut p4_ops, "F1", 10.5, 54.0, 620.0, "Because every highlight quad is stored in Normalized Page Space, zooming");
    push_text(&mut p4_ops, "F1", 10.5, 54.0, 604.0, "or resizing split panes multiplies only by scalar view scale s_i.");
    push_box(&mut p4_ops, 54.0, 550.0, 210.0, 24.0, [0.94, 0.94, 0.96], [0.40, 0.45, 0.55]);
    push_text(&mut p4_ops, "F2", 10.5, 64.0, 558.0, "<- Back to Theorem 1 (Page 2)");
    push_text(&mut p4_ops, "F2", 12.0, 54.0, 505.0, "References");
    push_text(&mut p4_ops, "F1", 10.0, 54.0, 488.0, "[1] Pinleaf Multi-View Reading Notes, 2026.");
    push_text(&mut p4_ops, "F1", 9.5, 54.0, 48.0, "Page 4 of 5");

    let p4_content = add_content_stream(&mut doc, p4_ops);
    let p4_link = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => vec![54.0.into(), 550.0.into(), 264.0.into(), 574.0.into()],
        "Border" => vec![0.into(), 0.into(), 0.into()],
        "A" => dictionary! {
            "S" => "GoTo",
            "D" => vec![Object::Reference(p2_id), "XYZ".into(), 0.0.into(), 760.0.into(), 0.0.into()],
        }
    });
    doc.objects.insert(
        p4_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
            "Resources" => Object::Reference(resources_id),
            "Contents" => Object::Reference(p4_content),
            "Annots" => vec![Object::Reference(p4_link)],
        }),
    );

    // --- Page 5: Rotated /Rotate 90 Summary Page ---
    // Map upright landscape coordinates (lx in 0..792, ly in 0..612) into raw portrait PDF space
    // (px = 612 - ly, py = lx) so /Rotate 90 CW displays upright horizontal landscape text.
    let mut p5_ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![0.0.into(), 1.0.into(), (-1.0).into(), 0.0.into(), 612.0.into(), 0.0.into()],
        ),
    ];
    push_text(&mut p5_ops, "F2", 15.0, 60.0, 540.0, "Rotated Landscape Appendix (/Rotate 90)");
    push_text(&mut p5_ops, "F1", 10.5, 60.0, 510.0, "This page tests 90-degree clockwise /Rotate handling for rendering,");
    push_text(&mut p5_ops, "F1", 10.5, 60.0, 492.0, "text selection hit-testing, and ISO-32000 QuadPoints round-tripping.");
    push_text(&mut p5_ops, "F1", 9.5, 60.0, 48.0, "Page 5 of 5");
    p5_ops.push(Operation::new("Q", vec![]));

    let p5_content = add_content_stream(&mut doc, p5_ops);
    doc.objects.insert(
        p5_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.0.into(), 0.0.into(), 612.0.into(), 792.0.into()],
            "Rotate" => 90,
            "Resources" => Object::Reference(resources_id),
            "Contents" => Object::Reference(p5_content),
        }),
    );

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 5,
            "Kids" => vec![
                Object::Reference(p1_id),
                Object::Reference(p2_id),
                Object::Reference(p3_id),
                Object::Reference(p4_id),
                Object::Reference(p5_id),
            ],
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut out = Vec::new();
    let _ = doc.save_to(&mut out);
    out
}

fn push_text(ops: &mut Vec<Operation>, font: &str, size: f32, x: f32, y: f32, text: &str) {
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("rg", vec![0.10.into(), 0.12.into(), 0.15.into()]));
    ops.push(Operation::new("Tf", vec![font.into(), size.into()]));
    ops.push(Operation::new(
        "Tm",
        vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), y.into()],
    ));
    ops.push(Operation::new("Tj", vec![Object::string_literal(text)]));
    ops.push(Operation::new("ET", vec![]));
}

fn push_rule(ops: &mut Vec<Operation>, x0: f32, y0: f32, x1: f32, y1: f32) {
    ops.push(Operation::new("q", vec![]));
    ops.push(Operation::new("RG", vec![0.45.into(), 0.48.into(), 0.55.into()]));
    ops.push(Operation::new("w", vec![1.0.into()]));
    ops.push(Operation::new("m", vec![x0.into(), y0.into()]));
    ops.push(Operation::new("l", vec![x1.into(), y1.into()]));
    ops.push(Operation::new("S", vec![]));
    ops.push(Operation::new("Q", vec![]));
}

fn push_box(
    ops: &mut Vec<Operation>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: [f32; 3],
    stroke: [f32; 3],
) {
    ops.push(Operation::new("q", vec![]));
    ops.push(Operation::new("rg", vec![fill[0].into(), fill[1].into(), fill[2].into()]));
    ops.push(Operation::new("RG", vec![stroke[0].into(), stroke[1].into(), stroke[2].into()]));
    ops.push(Operation::new("w", vec![1.2.into()]));
    ops.push(Operation::new("re", vec![x.into(), y.into(), w.into(), h.into()]));
    ops.push(Operation::new("B", vec![]));
    ops.push(Operation::new("Q", vec![]));
}

fn add_content_stream(doc: &mut Document, ops: Vec<Operation>) -> ObjectId {
    let encoded = Content { operations: ops }.encode().unwrap_or_default();
    doc.add_object(Stream::new(dictionary! {}, encoded))
}

fn add_preexisting_highlight(doc: &mut Document, rect: [f32; 4], note: &str) -> ObjectId {
    let [x0, y0, x1, y1] = rect;
    let ap_ops = vec![
        Operation::new("q", vec![]),
        Operation::new("gs", vec!["GS0".into()]),
        Operation::new("rg", vec![1.0.into(), 0.90.into(), 0.20.into()]),
        Operation::new("re", vec![0.0.into(), 0.0.into(), (x1 - x0).into(), (y1 - y0).into()]),
        Operation::new("f", vec![]),
        Operation::new("Q", vec![]),
    ];
    let ap_bytes = Content { operations: ap_ops }.encode().unwrap_or_default();
    let ap_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.0.into(), 0.0.into(), (x1 - x0).into(), (y1 - y0).into()],
            "Resources" => dictionary! {
                "ExtGState" => dictionary! {
                    "GS0" => dictionary! {
                        "Type" => "ExtGState",
                        "BM" => "Multiply",
                        "ca" => 0.35,
                    }
                }
            }
        },
        ap_bytes,
    ));

    doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Highlight",
        "NM" => Object::string_literal("pinleaf:11111111-1111-4111-8111-111111111111"),
        "Rect" => vec![x0.into(), y0.into(), x1.into(), y1.into()],
        "QuadPoints" => vec![
            x0.into(), y1.into(),
            x1.into(), y1.into(),
            x0.into(), y0.into(),
            x1.into(), y0.into(),
        ],
        "C" => vec![1.0.into(), 0.90.into(), 0.20.into()],
        "CA" => 0.35,
        "Contents" => Object::string_literal(note),
        "AP" => dictionary! {
            "N" => Object::Reference(ap_id),
        }
    })
}
