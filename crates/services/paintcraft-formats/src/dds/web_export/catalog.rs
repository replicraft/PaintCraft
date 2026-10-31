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
        let payload = self
            .payload_base(doc, 0)?
            .checked_add(catalog.phase_peak(self.spatial, 0)?)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| resource("Catalog payload overflow"))?;
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
