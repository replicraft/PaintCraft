//! Prepare one fixed typed context under the preleased Main slice, before OPEN.
use super::{
    candidate, capture::Capture7, frames::Frames7, operation, preparation::Preparation7,
    projection::Projection7, snapshot::Snapshot7,
};
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::Value,
    resources::{BudgetAccount, BudgetScope},
};
use paintcraft_editor::execute::metadata_clone;
use paintcraft_formats::{
    export::{
        layer_appearance::{self as shared, Inventory7, Requirements7},
        prepared_capabilities::Requirements,
    },
    pcnd::{self, CoverageFacts, RequirementsLimits},
};
use paintcraft_raster::{Admission, RasterLayer, coverage_mask::Limits};
use std::mem::size_of;

#[derive(Debug)]
pub(in crate::app::filter_editing) struct Prepared7 {
    pub capture: Capture7,
    pub frames: Frames7,
    pub operation: Value,
    pub operation_sha256: [u8; 32],
    pub source_sha256: [u8; 32],
    pub filter_digest: [u8; 32],
    pub resident: u64,
    pub metadata_bytes: u64,
    pub metadata_charge: u64,
    pub filter_masks: CoverageFacts,
    pub snapshot: Option<Snapshot7>,
    pub copy_spec: Option<Value>,
    pub copy_sha256: Option<[u8; 32]>,
    // Actual retained source Vec/account drops before these control declarations.
    _controls: BudgetAccount,
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source7 Main preparation scope exhausted",
    )
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn fixed_peak() -> Result<u64> {
    // Actual target types, including simultaneous local/returned fixed builders.
    // Existing bounded JSON control copies are separately named bootstrap class.
    [
        size_of::<Prepared7>(),
        size_of::<metadata_clone::Quote>(),
        2 * size_of::<Frames7>(),
        2 * size_of::<Projection7>(),
        size_of::<operation::Operation7>(),
        size_of::<Requirements7>(),
    ]
    .into_iter()
    .try_fold(0u64, |n, width| add(n, width as u64))
}
fn fits(scope: &BudgetScope, additional: u64, ceiling: u64) -> Result<()> {
    if add(scope.footprint().live, additional)? > ceiling {
        return Err(limit());
    }
    Ok(())
}
pub(in crate::app::filter_editing) fn prepare(
    capture: Capture7,
    prefix: Preparation7,
    admission: Admission,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    copy_spec: Option<Value>,
) -> Result<Prepared7> {
    check()?;
    let doc = &capture.document;
    prefix.validate_document(doc, check)?;
    let main_limit = (admission.paint_metadata_limit() as u64).min(64 << 20);
    let mut controls = scope.account();
    fits(scope, fixed_peak()?, main_limit)?;
    controls.resize(fixed_peak()?)?;
    // Quote walks are real metadata work; debit before calling their pure APIs.
    prefix.spend(doc.layers.len() as u64)?;
    let scratch = pcnd::portable_snapshot_scratch_quote(doc)?;
    fits(scope, scratch, main_limit)?;
    let snapshot = Snapshot7::encode(doc, main_limit as usize, scope, &prefix, check)?;
    let mut live_tiles = 0u64;
    for layer in doc.layers.values() {
        check()?;
        prefix.spend(1)?;
        live_tiles = add(live_tiles, layer.raster.resident_tiles() as u64)?;
    }
    let transient = pcnd::layer_appearance::transient_quote(doc.layers.len())?;
    fits(scope, transient, main_limit)?;
    let projection = Projection7::capture(
        snapshot.bytes(),
        doc,
        RequirementsLimits {
            layers: doc.layers.len(),
            tiles: live_tiles,
            metadata_bytes: 8 << 20,
            coverage: Limits::default(),
            work: capture.admission.base_work,
        },
        scope,
        &prefix,
        check,
    )?;
    let filter_digest = shared::filter_digest_checked(doc, scope, check, &|n| prefix.spend(n))?;
    let original = projection.coverage.source_inventory();
    let filter_masks = original.filter_coverage()?;
    let resident = projection
        .wire_tile_records
        .checked_mul(RasterLayer::TILE_BYTES as u64)
        .ok_or_else(limit)?;
    let metadata_bytes = if doc.source_metadata.is_some() {
        projection
            .wire_metadata_bytes
            .checked_sub(paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64)
            .ok_or_else(limit)?
    } else {
        0
    };
    let prepared_operation = operation::capture(&capture, &prefix)?;
    if let Some(FilterOp::Duplicate { .. }) = &capture.operation {
        let (layer, source_filter) = capture
            .operation
            .as_ref()
            .and_then(FilterOp::stable_uuid_refs)
            .ok_or_else(|| Error::new(ErrorCode::ConflictStaleBase, "Source7 duplicate target"))?;
        pcnd::layer_appearance::preflight_duplicate_counts(
            doc,
            layer,
            source_filter.ok_or_else(|| {
                Error::new(ErrorCode::ConflictStaleBase, "Source7 duplicate source")
            })?,
            check,
            &|n| prefix.spend(n),
        )?;
    }
    let staged = if let Some(batch) = &prepared_operation.batch {
        let quote = metadata_clone::quote_checked(doc, batch, check, &|n| prefix.spend(n))?;
        fits(scope, quote.peak_bytes, main_limit)?;
        Some(metadata_clone::stage_checked(
            doc,
            batch,
            admission,
            scope,
            check,
            &|n| prefix.spend(n),
        )?)
    } else {
        None
    };
    let candidate_doc = staged.as_ref().map(|s| s.document()).unwrap_or(doc);
    if staged.is_some() {
        prefix.validate_document(candidate_doc, check)?;
    }
    let provenance = if let Some(staged) = &staged {
        candidate::provenance(capture.operation.as_ref(), staged)?
    } else {
        pcnd::layer_appearance::CandidateProvenance::Unchanged
    };
    prefix.spend(candidate_doc.layers.len() as u64)?;
    let checked = || {
        check()?;
        prefix.spend(1)
    };
    let candidate_requirements = Requirements7::new(
        Requirements::capture(candidate_doc, &checked)?,
        shared::appearance_registration(),
        &checked,
    )?;
    let candidate_inventory: Inventory7 = projection.coverage.candidate_inventory_checked(
        doc,
        candidate_doc,
        provenance,
        scope,
        check,
        &|n| prefix.spend(n),
    )?;
    let source_requirements = projection.coverage.requirements().clone();
    let union_requirements = source_requirements.union(&candidate_requirements, &checked)?;
    // Only fixed facts/provenance are needed for view generations. Free candidate
    // metadata BEFORE sealing/OPEN; view-only updates never restage it on Main.
    drop(staged);
    let source_sha256 = projection.source_sha256;
    let metadata_charge = projection.metadata_charge;
    let copy_sha256 = if let Some(spec) = &copy_spec {
        let bytes = paintcraft_core::types::json::canonical(spec);
        if bytes.len() > 4096 {
            return Err(limit());
        }
        prefix.spend(bytes.len() as u64)?;
        use sha2::Digest;
        Some(sha2::Sha256::digest(&bytes).into())
    } else {
        None
    };
    let frames = Frames7::seal(
        prefix,
        (source_requirements, original),
        (candidate_requirements, candidate_inventory),
        union_requirements,
        provenance,
        check,
    )?;
    let result = Prepared7 {
        capture,
        frames,
        operation: prepared_operation.value,
        operation_sha256: prepared_operation.sha256,
        source_sha256,
        filter_digest,
        resident,
        metadata_bytes,
        metadata_charge,
        filter_masks,
        snapshot: Some(snapshot),
        copy_spec,
        copy_sha256,
        _controls: controls,
    };
    // No unused Stage/projection payload remains, only actual retained fields.
    // Reserved factory/JS credit lifecycle is owned outside this value.
    Ok(result)
}
