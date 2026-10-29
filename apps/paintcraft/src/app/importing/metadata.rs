//! One host-owned metadata realm spans tabs, pending imports and history leases.
use super::*;
impl App {
    pub(crate) fn source_metadata_budget(&self) -> paintcraft_runtime::SharedBudget {
        self.editing
            .as_ref()
            .map(|service| service.metadata_budget())
            .unwrap_or_else(|| self.source_metadata_budget.clone())
    }
    pub(crate) fn read_project_with_metadata(
        &self,
        bytes: &[u8],
    ) -> Result<paintcraft_document::Document> {
        let budget = self.source_metadata_budget();
        paintcraft_formats::pcnd::read_with_metadata_admission_checked(
            bytes,
            u64::MAX,
            paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            paintcraft_document::LAYER_COUNT_MAX,
            paintcraft_formats::pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: &budget,
            },
            &|| Ok(()),
        )
    }
}
