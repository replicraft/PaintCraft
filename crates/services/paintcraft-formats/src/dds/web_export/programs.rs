//! Generic priced program profile, finalized using actual snapshot length.
use super::*;
impl SourcePlan {
    pub(super) fn capture_programs(
        &mut self,
        doc: &Document,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        self.spatial = export::SourceSpatialContext::capture_programs(doc, 0, check)?;
        let programs = export::SourceProgramContext::capture(doc, self.spatial, check)?;
        let payload = self
            .program_payload_base(doc, 0)?
            .checked_add(programs.phase_peak(self.spatial, 0)?)
            .and_then(|n| n.checked_add(export::SourceProgramContext::CONTROL_COPY_BYTES))
            .ok_or_else(|| resource("Generic payload overflow"))?;
        self.spatial = self.spatial.programs_payload(payload)?;
        self.spatial.validate_programs(WORKER_PAYLOAD_BYTES_MAX)?;
        self.programs = Some(programs);
        Ok(())
    }
    /// Generic7 prices the decoded wire snapshot, not source Vec growth history.
    /// Keep the legacy4/5/6 payload_base and actual source admission unchanged.
    fn program_payload_base(&self, doc: &Document, source_bytes: usize) -> Result<u64> {
        let inventory = self.source.filter_inventory();
        let decoded = export::web_snapshot::decoded_filter_storage_bytes(inventory.filter_count())?;
        self.payload_base(doc, source_bytes)?
            .checked_sub(inventory.filter_storage_bytes())
            .and_then(|bytes| bytes.checked_add(decoded))
            .ok_or_else(|| resource("Generic decoded filter storage overflow"))
    }
    pub(super) fn finalized_program_region(
        &self,
        source_len: usize,
    ) -> Result<export::SourceSpatialContext> {
        if source_len == 0 || source_len > SOURCE_BYTES_MAX {
            return Err(resource("Generic source extent"));
        }
        let programs = self
            .programs
            .ok_or_else(|| invalid("Source plan is not generic profile7"))?;
        self.finalize_program_region(self.spatial, programs, source_len)
    }
    fn finalize_program_region(
        &self,
        region: export::SourceSpatialContext,
        programs: export::SourceProgramContext,
        source_len: usize,
    ) -> Result<export::SourceSpatialContext> {
        let region = region.finalize_programs_source(source_len, WORKER_PAYLOAD_BYTES_MAX)?;
        let pre_read = self
            .mask_context()
            .pre_read_quote(export::web_snapshot::pre_read_quote(
                source_len,
                SOURCE_LIMITS,
            )?)?
            .checked_add(self.metadata_context().retained_charge())
            .and_then(|n| n.checked_add(export::prepared_capabilities::CONTROL_BYTES))
            .ok_or_else(|| resource("Generic pre-read overflow"))?;
        let region = region.programs_payload(region.payload_bytes().max(pre_read))?;
        region.validate_programs(WORKER_PAYLOAD_BYTES_MAX)?;
        programs.validate_region(region)?;
        Ok(region)
    }
    /// Finalize generic7 from the actual authenticated frozen PCND bytes.
    /// The length-only API remains for actual decoded-worker revalidation.
    pub fn snapshot_contexts(
        &self,
        source: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<export::SourceSnapshotContexts> {
        let programs = self
            .programs
            .ok_or_else(|| invalid("Source plan is not generic profile7"))?;
        if source.is_empty() || source.len() > SOURCE_BYTES_MAX {
            return Err(resource("Generic source extent"));
        }
        let (resident, source_sha256) =
            export::snapshot_resident_bytes(source, RESIDENT_BYTES_MAX, check)?;
        // Normalize BEFORE source-length addition and pre-read max. Subtracting
        // after that max could reduce an independently required parser quote.
        let payload = self
            .spatial
            .payload_bytes()
            .checked_sub(self.source.resident_bytes())
            .and_then(|bytes| bytes.checked_add(resident))
            .ok_or_else(|| resource("Generic snapshot resident extent"))?;
        let region = self.spatial.programs_payload(payload)?;
        let region = self.finalize_program_region(region, programs, source.len())?;
        check()?;
        Ok(export::SourceSnapshotContexts {
            region: region.encode_programs(),
            programs: programs.encode(),
            source_sha256,
        })
    }
    /// Exact descriptive facts from this same producer ABI, after serialization.
    pub fn program_context(&self, source_len: usize) -> Result<[u8; 128]> {
        self.finalized_program_region(source_len)?;
        Ok(self
            .programs
            .ok_or_else(|| invalid("Missing generic profile"))?
            .encode())
    }
}
