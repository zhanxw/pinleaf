pub mod annotation;
pub mod coords;
pub mod selection;
pub mod viewport;

pub use annotation::{
    Annotation, AnnotationDiff, AnnotationId, AnnotationKind, AnnotationStore, RgbaColor,
    highlight_category_spec, normalize_category_char,
};
pub use coords::{NormPoint, NormQuad, NormRect, PageIndex, PageRotation, PageTransform, PdfPoint};
pub use selection::{PageCharPos, TextSelectionRange};
pub use viewport::{
    FreezeState, JumpHistory, PortalLink, ReadingAnchor, ReadingTrailGraph, TrailNode, ViewportId,
    ZoomSpec, find_active_portal,
};
