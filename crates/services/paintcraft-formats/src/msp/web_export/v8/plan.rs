use super::*;
/// Version-aware snapshot admission. Closed legacy SourcePlan remains unchanged.
#[derive(Clone, Debug)]
pub struct NoiseSourcePlan {
    source: SnapshotPlan,
    version: Version,
    region: export::SourceNoiseRegionContext,
    programs: export::SourceNoiseProgramContext,
    workload: Workload,
    base_payload: u64,
}
impl NoiseSourcePlan {
    pub fn size(&self) -> (u32, u32) {
        self.source.size()
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn settings(&self) -> [u8; 8] {
        settings(self.version)
    }
    pub fn metadata_context(&self) -> export::SourceMetadataContext {
        self.source.metadata_context()
    }
    pub fn mask_context(&self) -> export::SourceMaskContext {
        self.source.mask_context()
    }
    pub fn filter_digest(&self) -> [u8; 32] {
        self.source.filter_inventory().digest()
    }
    pub fn bake_intent(&self) -> u8 {
        self.source.bake_intent()
    }
    pub fn source_flags(&self) -> u8 {
        self.source.flags()
    }
    pub fn validate_policies(&self, doc: &Document) -> Result<()> {
        self.source.validate_policy(doc)
    }
    pub fn snapshot(&self, doc: &Document) -> Result<Vec<u8>> {
        self.source.snapshot(doc, |size| {
            RgbaLayout::new(self.version, size.0, size.1).map(|_| ())
        })
    }
    pub fn capture_with_policies_checked(
        doc: &Document,
        version: Version,
        policies: export::CopyPolicies,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let source = SnapshotPlan::capture_with_policies(
            doc,
            NOISE_SOURCE_LIMITS,
            policies.source_metadata,
            policies.filter_bake,
            export::metadata_policy::Target::new(
                crate::registry::Family::Msp,
                crate::registry::Operation::ExportFlat,
            ),
        )?;
        let layout = RgbaLayout::new(version, doc.canvas.0, doc.canvas.1)?;
        encode_extra(version, doc.canvas)?;
        // Composite/counting scratch stays in the baseline throughout rendering
        // and encoding. Appearance workspace and packed/output extras are priced
        // as disjoint phases by phase_payload, never summed as concurrent scratch.
        let base_payload = export::msp::render_reservation_bytes(layout)?
            .checked_add(source.resident_bytes())
            .and_then(|n| n.checked_add(source.metadata_context().retained_charge()))
            .and_then(|n| n.checked_add(source.mask_context().decode_bytes()))
            .and_then(|n| {
                n.checked_add(
                    export::web_snapshot::decoded_filter_storage_bytes(
                        source.filter_inventory().filter_count(),
                    )
                    .ok()?,
                )
            })
            .and_then(|n| n.checked_add(control_bytes().ok()?))
            .and_then(|n| {
                n.checked_add(
                    (8 << 20) + (1 << 20) + ENCODED_BYTES_MAX as u64 + 2 * PREVIEW_BYTES_MAX as u64,
                )
            })
            .ok_or_else(|| resource("MSP8 source baseline"))?;
        let region = export::SourceNoiseRegionContext::capture(doc, base_payload, workload, check)?;
        let programs = export::SourceNoiseProgramContext::capture(doc, region, workload, check)?;
        check()?;
        Ok(Self {
            source,
            version,
            region,
            programs,
            workload,
            base_payload,
        })
    }
    pub fn matches_context(&self, h: &Header) -> bool {
        h.version == self.version
            && h.workload == self.workload
            && h.programs == self.programs
            && h.spatial.work() == self.region.work()
            && h.spatial.workspace_bytes() == self.region.workspace_bytes()
            && h.spatial.stored_noise() == self.region.stored_noise()
            && h.spatial.stored_spatial() == self.region.stored_spatial()
    }
    pub fn workload(&self) -> Workload {
        self.workload
    }
    pub fn protocol_version(&self) -> u16 {
        8
    }
    pub fn source_state(&self) -> Uuid {
        self.source.state()
    }
    pub fn snapshot_contexts(
        &self,
        source: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(
        export::SourceNoiseRegionContext,
        export::SourceNoiseProgramContext,
        [u8; 32],
    )> {
        if source.is_empty() || source.len() > SOURCE_BYTES_MAX {
            return Err(resource("MSP8 source extent"));
        }
        let (resident, sha) = export::snapshot_resident_bytes(source, RESIDENT_BYTES_MAX, check)?;
        let (region, programs) = self.finalize(source.len(), resident)?;
        Ok((region, programs, sha))
    }
    pub(super) fn finalize(
        &self,
        len: usize,
        resident: u64,
    ) -> Result<(
        export::SourceNoiseRegionContext,
        export::SourceNoiseProgramContext,
    )> {
        if len == 0 || len > SOURCE_BYTES_MAX || resident > RESIDENT_BYTES_MAX {
            return Err(resource("MSP8 snapshot extent"));
        }
        let base = self
            .base_payload
            .checked_sub(self.source.resident_bytes())
            .and_then(|n| n.checked_add(resident))
            .ok_or_else(|| resource("MSP8 resident normalization"))?;
        let region = self
            .region
            .with_base_payload(base, self.workload)?
            .finalize_source(len, self.workload, WORKER_PAYLOAD_BYTES_MAX)?;
        self.programs.phase_payload(
            region,
            self.workload,
            encode_extra(self.version, self.size())?,
            WORKER_PAYLOAD_BYTES_MAX,
        )?;
        Ok((region, self.programs))
    }
}
