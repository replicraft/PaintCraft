//! Actual encoded-source demand, paid by the original counted preparation.
use super::{
    BlendPlanRecordUpperBounds, CountedBlendSnapshot, CountedBlendSnapshotBytes, mismatch, step,
};
use crate::pcnd::{
    CheckedContainer, PortableSource8Limits, RequirementsLimits, RetainedSourceLayoutSummary8,
    layer_blend_appearance::{self as appearance, Source8ProbeWorkFacts},
};
use paintcraft_core::types::Result;
use std::mem::size_of;
mod controls;
use controls::{MAIN_MAX, QUERY_CONTROL};

/// Target-specific scalar source demand. Never a Worker role grant or adoption.
/// The reader still independently authenticates/reconciles the actual source.
/// C/K assembly/carrier, factories, renderer/service/output controls are excluded.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendSourceDemand {
    /// SHA256 of the complete encoded source, including its trailer.
    pub source_sha256: [u8; 32],
    /// Original immutable resident-record bounds used for plan bootstrap.
    pub record_upper: BlendPlanRecordUpperBounds,
    /// Authenticated census and defining target storage bounds.
    pub retained: RetainedSourceLayoutSummary8,
    /// Exact encoded length, separately owned from P controls and scratch.
    pub encoded_bytes: u64,
    /// Actual sum of partial wire-block decoded pixels, distinct from full Tile Q.
    pub wire_pixels: u64,
    /// Actual Plan plus its exact layer/block records, without helper scratch.
    pub p_plan_records: u64,
    /// P peak while plan records, scan and retained census coexist.
    pub p_plan_census_peak: u64,
    /// P peak while plan records and the complete source probe coexist.
    pub p_plan_probe_peak: u64,
    /// Maximum P phase, excluding encoded source and Main query controls.
    pub p_peak: u64,
    /// Main bootstrap bound uses resident tiles; serialized records can be fewer.
    pub bootstrap_p_peak: u64,
    /// Encoded document identity reconciled with the counted source.
    pub document_id: [u8; 16],
    /// Encoded private document state, distinct from a publication ticket.
    pub state_id: [u8; 16],
    /// Encoded width and height in native pixels.
    pub canvas: [u32; 2],
    /// Stored nonNormal intent, including hidden or zero-opacity layers.
    pub stored_non_normal: bool,
    /// Stored Noise requirements, including disabled filters.
    pub stored_noise: bool,
    /// A native forecast is not a wasm32 layout forecast.
    pub pointer_bytes: u8,
}
impl CountedBlendSnapshot<'_> {
    /// Encode and forecast those SAME bytes, using only bound originals.
    ///
    /// The original reserved Main scope remains <=64MiB. Checks of its remaining
    /// room only refuse: no capacity/account is enlarged from observed usage.
    /// Plan/census/probe and this temporary32768 declaration overlap the actual
    /// encoded owner before returning. All borrowed helpers retire before return.
    /// The existing opaque bytes retain their original factory after this fact.
    /// Returned tuple/scalar controls belong to the caller. This is not complete
    /// Worker C/K/H/F/W/O admission, an execution token, or a decoded domain.
    ///
    /// Original callback errors move unchanged; every retry keeps its bound
    /// original spend callback and previously paid debt. No new job is created.
    /// The Main host must retain its existing external broker credit through the
    /// returned bytes' lifetime. Scope capacity equality does not prove funding
    /// ancestry or replace that host responsibility.
    ///
    /// No replacement factory or callbacks can be supplied:
    /// ```compile_fail
    /// use paintcraft_formats::pcnd::CountedBlendSnapshot;
    /// use paintcraft_core::types::resources::BudgetScope;
    /// fn replace(fact: &CountedBlendSnapshot<'_>, other: &BudgetScope) {
    ///     let _ = fact.encode_with_source_forecast_checked(other);
    /// }
    /// ```
    ///
    /// # Errors
    /// Returns the original callback/spend/allocator denial, ResourceLimit for
    /// insufficient original Main capacity or unsupported target controls, and
    /// the existing codec/identity errors if authentication or source checks fail.
    /// Escaped owned diagnostics are not a whole-heap accounting guarantee.
    pub fn encode_with_source_forecast_checked(
        &self,
    ) -> Result<(CountedBlendSnapshotBytes, BlendSourceDemand)> {
        (self.spend)(64)?;
        // Complete containing slots/moves BEFORE even the defining guard result.
        (self.spend)(2 * QUERY_CONTROL + 16384)?;
        controls::validate()?;
        (self.spend)(112)?;
        let capacity = self.scope.capacity_snapshot();
        if !capacity.reserved || capacity.ceiling > MAIN_MAX {
            return Err(controls::exhausted());
        }
        let mut control = self.scope.account();
        control.resize(QUERY_CONTROL)?;
        let upper = self.plan_record_upper_bounds_checked()?;
        let bytes = self.encode()?;
        (self.spend)(64)?;
        let bootstrap = controls::peaks(upper.layers, upper.tiles)?;
        controls::ensure_room(self, bootstrap.peak)?;
        // Both source authentication hashes are paid before touching source.
        (self.spend)(2 * bytes.len() as u64)?;
        let auth_check = || step(self.check, self.spend);
        let (checked, source_sha256) =
            CheckedContainer::verify_with_source_hash(bytes.as_bytes(), &auth_check)?;
        let limits = PortableSource8Limits {
            layers: usize::try_from(upper.layers).map_err(|_| controls::exhausted())?,
            tiles: upper.tiles,
            metadata_bytes: MAIN_MAX as usize,
            ..Default::default()
        };
        let plan =
            checked.portable_source8_plan_checked(limits, self.scope, self.check, self.spend)?;
        let retained = {
            let recipe = plan.retained_source_layout_checked()?;
            (self.spend)(168)?;
            *recipe.summary()
        };
        (self.spend)(64)?;
        let actual = controls::peaks(retained.layers(), retained.tiles())?;
        if retained.layers() != upper.layers || retained.tiles() > upper.tiles {
            return Err(mismatch());
        }
        let wire_pixels = self.wire_pixels(&plan, retained.layers())?;
        let header = plan.header()?;
        if header.document_id != self.doc.document_id
            || header.state_id != self.doc.state_id
            || header.canvas != self.doc.canvas
        {
            return Err(mismatch());
        }
        (self.spend)(64)?;
        let metadata = retained
            .metadata_bytes()
            .checked_sub(retained.profile_bytes())
            .ok_or_else(mismatch)?;
        let units = appearance::source8_probe_supplemental_work_quote(Source8ProbeWorkFacts {
            layers: retained.layers(),
            filters: retained.filters(),
            masks: retained.mask_present(),
            bands: retained.bands(),
            runs: retained.runs(),
            metadata,
        })?;
        (self.spend)(units)?;
        let demand = {
            let source = checked.probe_layer_blend_appearance_checked(
                RequirementsLimits {
                    layers: limits.layers,
                    tiles: limits.tiles,
                    metadata_bytes: limits.metadata_bytes,
                    coverage: limits.coverage,
                    work: limits.work,
                },
                self.scope,
                self.check,
                self.spend,
            )?;
            (self.spend)(64)?;
            let expected_metadata = retained
                .attachment_count()
                .checked_mul(192)
                .and_then(|n| n.checked_add(retained.profile_bytes()))
                .ok_or_else(mismatch)?;
            if source.wire_tile_records() != retained.tiles()
                || source.wire_metadata_bytes() != expected_metadata
            {
                return Err(mismatch());
            }
            // Separate actual unique-registry lookup pass; not hidden in probe work.
            let pairs = source.requirements().requirements().len() as u64;
            (self.spend)(64 + 16384 * pairs)?;
            let stored_noise = source.requirements().requires_noise()?;
            BlendSourceDemand {
                source_sha256,
                record_upper: upper,
                retained,
                encoded_bytes: bytes.len() as u64,
                wire_pixels,
                p_plan_records: actual.records,
                p_plan_census_peak: actual.census,
                p_plan_probe_peak: actual.probe,
                p_peak: actual.peak,
                bootstrap_p_peak: bootstrap.peak,
                document_id: *header.document_id.as_bytes(),
                state_id: *header.state_id.as_bytes(),
                canvas: [header.canvas.0, header.canvas.1],
                stored_non_normal: true,
                stored_noise,
                pointer_bytes: size_of::<usize>() as u8,
            }
        };
        drop(plan);
        drop(control);
        Ok((bytes, demand))
    }
    fn wire_pixels(&self, plan: &crate::pcnd::PortableSource8Plan<'_>, layers: u64) -> Result<u64> {
        let mut total = 0u64;
        for i in 0..layers as usize {
            (self.spend)(1024)?;
            let layer = plan.layer(i)?;
            for j in 0..layer.block_count() {
                (self.spend)(1024)?;
                let block = layer.block(j)?;
                total = total
                    .checked_add(u64::from(block.decoded_bytes) / 4)
                    .ok_or_else(controls::exhausted)?;
            }
        }
        Ok(total)
    }
}
#[cfg(test)]
mod tests;
