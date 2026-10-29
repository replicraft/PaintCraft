//! Private new-document candidates; hosts still authorize adoption and losses.
use super::Decoded;
use paintcraft_core::types::Result;
use paintcraft_document::{Document, RasterConversionKey, SourceMetadata};
pub const ICC_RETAINED_UNAPPLIED: &str = "icc-source-retained-unapplied";
pub const RESOLUTION_RETAINED_INACTIVE: &str = "resolution-source-retained-inactive";

pub struct Prepared {
    pub image: Decoded,
    pub attachment: Option<SourceMetadata>,
    pub document_key: RasterConversionKey,
}
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedRaster")
            .field("format", &self.image.format)
            .field("dimensions", &(self.image.width, self.image.height))
            .field("rgba_bytes", &self.image.rgba.len())
            .field("acknowledgements", &self.image.losses)
            .field("attachment", &self.attachment)
            .field("document_key", &self.document_key.hex())
            .finish()
    }
}
impl Prepared {
    /// Use the caller's raster realm, then check actual initial PCND save
    /// viability. This does not promise arbitrary later edits remain savable.
    pub fn document_with_admission(
        &self,
        admission: &mut paintcraft_raster::Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Document> {
        let document = paintcraft_document::import_image_with_admission_checked(
            self.image.width,
            self.image.height,
            &self.image.rgba,
            &self.document_key.hex(),
            admission,
            check,
        )?;
        let document = document.with_source_metadata_checked(self.attachment.clone(), check)?;
        crate::pcnd::check_compact_metadata_admission(&document, check)?;
        Ok(document)
    }
}
