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
        let layout = RgbaLayout::new(Version::RowRleV2, doc.canvas.0, doc.canvas.1)?;
        let encode_extra = (layout.packed_bytes() as u64)
            .checked_add(ENCODED_BYTES_MAX as u64)
            .ok_or_else(|| resource("Generic MSP phase overflow"))?;
        let common = self
            .payload_base(doc, 0)?
            .checked_sub(encode_extra.saturating_sub(self.spatial.workspace_bytes()))
            .ok_or_else(|| resource("Generic MSP baseline overflow"))?;
        let payload = common
            .checked_add(programs.phase_peak(self.spatial, encode_extra)?)
            .and_then(|n| n.checked_add(export::SourceProgramContext::CONTROL_COPY_BYTES))
            .ok_or_else(|| resource("Generic MSP payload overflow"))?;
        self.spatial = self.spatial.programs_payload(payload)?;
        self.spatial.validate_programs(WORKER_PAYLOAD_BYTES_MAX)?;
        self.programs = Some(programs);
        Ok(())
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
        let region = self
            .spatial
            .finalize_programs_source(source_len, WORKER_PAYLOAD_BYTES_MAX)?;
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
    /// Exact descriptive facts from this same producer ABI, after serialization.
    pub fn program_context(&self, source_len: usize) -> Result<[u8; 128]> {
        self.finalized_program_region(source_len)?;
        Ok(self
            .programs
            .ok_or_else(|| invalid("Missing generic profile"))?
            .encode())
    }
}
