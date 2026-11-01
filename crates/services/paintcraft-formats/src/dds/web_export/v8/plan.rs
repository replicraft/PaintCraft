use super::*;
/// Separate plan: no size/branch change to the closed old SourcePlan.
#[derive(Clone, Debug)]
pub struct NoiseSourcePlan {
    legacy: SourcePlan,
    region: export::SourceNoiseRegionContext,
    programs: export::SourceNoiseProgramContext,
    workload: Workload,
    base_payload: u64,
}
impl NoiseSourcePlan {
    pub fn size(&self) -> (u32, u32) {
        self.legacy.size()
    }
    pub fn settings(&self) -> [u8; 8] {
        self.legacy.settings()
    }
    pub fn metadata_context(&self) -> export::SourceMetadataContext {
        self.legacy.metadata_context()
    }
    pub fn mask_context(&self) -> export::SourceMaskContext {
        self.legacy.mask_context()
    }
    pub fn filter_digest(&self) -> [u8; 32] {
        self.legacy.filter_digest()
    }
    pub fn bake_intent(&self) -> u8 {
        self.legacy.bake_intent()
    }
    pub fn source_flags(&self) -> u8 {
        self.legacy.source_flags()
    }
    pub fn validate_policies(&self, doc: &Document) -> Result<()> {
        self.legacy.validate_policies(doc)
    }
    pub fn snapshot(&self, doc: &Document) -> Result<Vec<u8>> {
        self.legacy.snapshot(doc)
    }
    pub fn capture_with_policies_checked(
        doc: &Document,
        options: TextureOptions,
        policies: export::CopyPolicies,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let source = SnapshotPlan::capture_with_policies(
            doc,
            SOURCE_LIMITS,
            policies.source_metadata,
            policies.filter_bake,
            export::metadata_policy::Target::new(
                crate::registry::Family::Dds,
                crate::registry::Operation::ExportFlat,
            ),
        )?;
        let legacy = SourcePlan {
            source,
            spatial: Default::default(),
            catalog: None,
            programs: None,
            settings: encode_settings(options, doc.canvas)?,
        };
        let inventory = legacy.source.filter_inventory();
        let base_payload = legacy
            .payload_base(doc, 0)?
            .checked_sub(inventory.filter_storage_bytes())
            .and_then(|n| {
                n.checked_add(
                    export::web_snapshot::decoded_filter_storage_bytes(inventory.filter_count())
                        .ok()?,
                )
            })
            .and_then(|n| n.checked_add(control_bytes().ok()?))
            .ok_or_else(|| resource("DDS8 snapshot base"))?;
        let region = export::SourceNoiseRegionContext::capture(doc, base_payload, workload, check)?;
        let programs = export::SourceNoiseProgramContext::capture(doc, region, workload, check)?;
        Ok(Self {
            legacy,
            region,
            programs,
            workload,
            base_payload,
        })
    }
    pub fn matches_context(&self, h: &Header) -> bool {
        h.workload == self.workload
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
        self.legacy.source.state()
    }
    /// Producer finalization binds actual authenticated records, not projection.
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
            return Err(resource("DDS8 source extent"));
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
            return Err(resource("DDS8 snapshot extent"));
        }
        let base = self
            .base_payload
            .checked_sub(self.legacy.source.resident_bytes())
            .and_then(|n| n.checked_add(resident))
            .ok_or_else(|| resource("DDS8 resident normalization"))?;
        let region = self
            .region
            .with_base_payload(base, self.workload)?
            .finalize_source(len, self.workload, WORKER_PAYLOAD_BYTES_MAX)?;
        self.programs
            .phase_payload(region, self.workload, 0, WORKER_PAYLOAD_BYTES_MAX)?;
        Ok((region, self.programs))
    }
}
