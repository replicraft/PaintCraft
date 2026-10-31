//! Source-bound initial catalog profile, finalized using actual snapshot length.
use super::*;
impl SourcePlan {
    pub(super) fn capture_catalog(
        &mut self,
        doc: &Document,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        self.spatial = export::SourceSpatialContext::capture_catalog(doc, 0, check)?;
        let catalog = export::SourceCatalogContext::capture(doc, self.spatial, check)?;
        let layout = RgbaLayout::new(Version::RowRleV2, doc.canvas.0, doc.canvas.1)?;
        let encode_extra = (layout.packed_bytes() as u64)
            .checked_add(ENCODED_BYTES_MAX as u64)
            .ok_or_else(|| resource("Catalog MSP phase overflow"))?;
        let common = self
            .payload_base(doc, 0)?
            .checked_sub(encode_extra.saturating_sub(self.spatial.workspace_bytes()))
            .ok_or_else(|| resource("Catalog MSP baseline overflow"))?;
        let payload = common
            .checked_add(catalog.phase_peak(self.spatial, encode_extra)?)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| resource("Catalog MSP payload overflow"))?;
        self.spatial = self.spatial.catalog_payload(payload)?;
        self.spatial.validate_catalog(WORKER_PAYLOAD_BYTES_MAX)?;
        self.catalog = Some(catalog);
        Ok(())
    }
    pub(super) fn finalized_catalog_region(
        &self,
        source_len: usize,
    ) -> Result<export::SourceSpatialContext> {
        if source_len == 0 || source_len > SOURCE_BYTES_MAX {
            return Err(resource("Catalog source extent"));
        }
        let catalog = self
            .catalog
            .ok_or_else(|| invalid("Source plan is not catalog profile6"))?;
        let region = self
            .spatial
            .finalize_catalog_source(source_len, WORKER_PAYLOAD_BYTES_MAX)?;
        catalog.validate_region(region)?;
        Ok(region)
    }
    /// Exact descriptive facts from this same producer ABI, after serialization.
    pub fn catalog_context(&self, source_len: usize) -> Result<[u8; 64]> {
        self.finalized_catalog_region(source_len)?;
        Ok(self
            .catalog
            .ok_or_else(|| invalid("Missing catalog profile"))?
            .encode())
    }
}
