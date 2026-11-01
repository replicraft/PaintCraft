//! Provenance comes from the real Editor staged alias, never a predicted UUID.
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_editor::execute::metadata_clone::StagedMetadata;
use paintcraft_formats::pcnd::layer_appearance::CandidateProvenance;

pub(in crate::app::filter_editing) fn provenance(
    operation: Option<&FilterOp>,
    staged: &StagedMetadata,
) -> Result<CandidateProvenance> {
    let Some(operation) = operation else {
        return Ok(CandidateProvenance::Unchanged);
    };
    let (layer, source_filter) = operation.stable_uuid_refs().ok_or_else(invalid)?;
    if matches!(operation, FilterOp::Mask { .. }) {
        return Err(invalid());
    }
    let FilterOp::Duplicate { alias, .. } = operation else {
        return Ok(CandidateProvenance::Unchanged);
    };
    let source_filter = source_filter.ok_or_else(invalid)?;
    let (actual_layer, new_filter) = staged
        .filter_aliases()
        .get(alias)
        .copied()
        .ok_or_else(invalid)?;
    if actual_layer != layer || new_filter == source_filter {
        return Err(invalid());
    }
    let candidate_layer = staged.document().layers.get(&layer).ok_or_else(invalid)?;
    if !candidate_layer.filters.iter().any(|f| f.id() == new_filter) {
        return Err(invalid());
    }
    Ok(CandidateProvenance::Duplicate {
        layer,
        source_filter,
        new_filter,
    })
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Source7 staged filter provenance changed",
    )
}
