use std::sync::Arc;

use hayro_syntax::Pdf;
use hayro_syntax::page::Rotation;
use pinleaf_core::{PageIndex, PageRotation, PageTransform};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("failed to parse PDF: {0}")]
    ParseFailed(String),
    #[error("PDF is password-protected or decryption failed")]
    Encrypted,
    #[error("page index {0} out of bounds (total pages: {1})")]
    PageOutOfBounds(u32, usize),
    #[error("failed to render page {0}: {1}")]
    RenderFailed(u32, String),
}

#[derive(Clone, Debug)]
pub struct PageMeta {
    pub index: PageIndex,
    pub transform: PageTransform,
    pub norm_width: f32,
    pub norm_height: f32,
}

/// Thread-safe, zero-copy handle around a loaded PDF document.
/// Shared across all viewports and OS windows in a `DocumentSession`.
pub struct PdfDocumentHandle {
    raw_bytes: Arc<Vec<u8>>,
    pdf: Pdf,
    pages: Vec<PageMeta>,
}

impl PdfDocumentHandle {
    pub fn from_bytes(raw_bytes: Arc<Vec<u8>>) -> Result<Self, EngineError> {
        let pdf = Pdf::new(raw_bytes.clone()).map_err(|err| match err {
            hayro_syntax::LoadPdfError::Decryption(_) => EngineError::Encrypted,
            hayro_syntax::LoadPdfError::Invalid => {
                EngineError::ParseFailed("invalid or unsupported PDF structure".to_string())
            }
        })?;

        let mut pages = Vec::with_capacity(pdf.pages().len());
        for (idx, page) in pdf.pages().iter().enumerate() {
            let rect = page.intersected_crop_box();
            let crop_box = [
                rect.x0 as f32,
                rect.y0 as f32,
                rect.x1 as f32,
                rect.y1 as f32,
            ];
            let rotation = match page.rotation() {
                Rotation::None => PageRotation::Deg0,
                Rotation::Horizontal => PageRotation::Deg90Cw,
                Rotation::Flipped => PageRotation::Deg180,
                Rotation::FlippedHorizontal => PageRotation::Deg270Cw,
            };
            let transform = PageTransform::new(crop_box, rotation);
            let (norm_width, norm_height) = page.render_dimensions();
            pages.push(PageMeta {
                index: PageIndex(idx as u32),
                transform,
                norm_width: norm_width.max(1.0),
                norm_height: norm_height.max(1.0),
            });
        }

        if pages.is_empty() {
            return Err(EngineError::ParseFailed("PDF contains zero pages".to_string()));
        }

        Ok(Self {
            raw_bytes,
            pdf,
            pages,
        })
    }

    pub fn raw_bytes(&self) -> &Arc<Vec<u8>> {
        &self.raw_bytes
    }

    pub fn pdf(&self) -> &Pdf {
        &self.pdf
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page_metas(&self) -> &[PageMeta] {
        &self.pages
    }

    pub fn page_meta(&self, page: PageIndex) -> Result<&PageMeta, EngineError> {
        self.pages
            .get(page.as_usize())
            .ok_or(EngineError::PageOutOfBounds(page.0, self.pages.len()))
    }

    pub fn page_transforms(&self) -> Vec<PageTransform> {
        self.pages.iter().map(|m| m.transform).collect()
    }
}
