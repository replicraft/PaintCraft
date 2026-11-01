use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceNoiseProgramContext {
    required: u32,
    stored: u32,
    enabled: u32,
    unique: u32,
    compile: u64,
    retained: u64,
    preparation: u64,
    evaluation: u64,
    work: WorkComponents,
    counter_bytes: u64,
    metadata_bytes: u64,
    required_digest: [u8; 32],
    resolution_digest: [u8; 32],
    workload_digest: [u8; 32],
    capability_digest: [u8; 32],
}
impl SourceNoiseProgramContext {
    pub fn capture(
        doc: &Document,
        region: SourceNoiseRegionContext,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        region.validate(workload, PAYLOAD_MAX)?;
        if doc.layers.len() > 8 {
            return Err(limit());
        }
        let requirements = Requirements::capture(doc, check)?;
        requirements.validate_contract(RegistryContract::ScopedNoise2, check)?;
        let (required_digest, resolution_digest) = requirements.digests(check)?;
        let controls = NoiseContextControlQuote::capture()?;
        let mut value = Self {
            required: u32::try_from(requirements.len()).map_err(|_| limit())?,
            required_digest,
            resolution_digest,
            workload_digest: workload.digest()?,
            capability_digest: capability(&requirements, workload, check)?,
            counter_bytes: controls.counter_bytes,
            metadata_bytes: controls.metadata_bytes()?,
            ..Self::default()
        };
        for layer in doc.layers.values() {
            check()?;
            for filter in layer.filters.iter() {
                check()?;
                value.stored = value.stored.checked_add(1).ok_or_else(limit)?;
                value.enabled = value
                    .enabled
                    .checked_add(u32::from(filter.enabled()))
                    .ok_or_else(limit)?;
            }
        }
        let q = paintcraft_render::prepared::quote_document_programs(doc, check)?;
        value.unique = u32::try_from(q.unique_programs).map_err(|_| limit())?;
        value.compile = q.compile_work;
        value.retained = q.retained_bytes;
        value.preparation = q.preparation_peak_bytes;
        value.evaluation = value
            .retained
            .checked_add(region.workspace_bytes())
            .and_then(|n| n.checked_add(value.counter_bytes))
            .ok_or_else(limit)?;
        value.work = region.work().checked_add(WorkComponents {
            base: value.compile,
            noise: 0,
        })?;
        value.validate_region(region, workload)?;
        value.validate_current_abi()?;
        check()?;
        Ok(value)
    }
    pub fn work(self) -> WorkComponents {
        self.work
    }
    pub fn compile_work(self) -> u64 {
        self.compile
    }
    pub fn retained_bytes(self) -> u64 {
        self.retained
    }
    pub fn unique_programs(self) -> u32 {
        self.unique
    }
    pub fn capability_digest(self) -> [u8; 32] {
        self.capability_digest
    }
    pub fn validate_requirements(
        self,
        required: &Requirements,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        required.validate_contract(RegistryContract::ScopedNoise2, check)?;
        let (r, s) = required.digests(check)?;
        if self.required as usize != required.len()
            || self.required_digest != r
            || self.resolution_digest != s
            || self.workload_digest != workload.digest()?
            || self.capability_digest != capability(required, workload, check)?
        {
            return Err(bad());
        }
        Ok(())
    }
    pub fn validate_current_abi(self) -> Result<()> {
        let q = NoiseContextControlQuote::capture()?;
        if self.counter_bytes != q.counter_bytes || self.metadata_bytes != q.metadata_bytes()? {
            return Err(bad());
        }
        Ok(())
    }
    pub fn workspace_peak(self) -> Result<u64> {
        Ok(self.evaluation.max(
            self.preparation
                .checked_add(self.counter_bytes)
                .ok_or_else(limit)?,
        ))
    }
    /// Encoder scratch shares the phase, not a second independent byte realm.
    /// Caller must bind finalized snapshot facts before any codec adoption.
    pub fn phase_payload(
        self,
        region: SourceNoiseRegionContext,
        workload: Workload,
        encode_extra: u64,
        cap: u64,
    ) -> Result<u64> {
        self.validate_region(region, workload)?;
        self.validate_current_abi()?;
        if region.source_bytes() == 0 || cap > PAYLOAD_MAX {
            return Err(bad());
        }
        let base = region
            .payload_bytes()
            .checked_sub(region.workspace_bytes())
            .ok_or_else(limit)?;
        let eval = self
            .retained
            .checked_add(region.workspace_bytes().max(encode_extra))
            .and_then(|n| n.checked_add(self.counter_bytes))
            .ok_or_else(limit)?;
        let peak = eval.max(
            self.preparation
                .checked_add(self.counter_bytes)
                .ok_or_else(limit)?,
        );
        base.checked_add(peak)
            .and_then(|n| n.checked_add(self.metadata_bytes))
            .filter(|n| *n <= cap)
            .ok_or_else(limit)
    }
    pub fn validate_region(
        self,
        region: SourceNoiseRegionContext,
        workload: Workload,
    ) -> Result<()> {
        region.validate(workload, PAYLOAD_MAX)?;
        workload.check(self.work)?;
        if self.required > 256
            || self.stored > STORED_MAX
            || self.enabled > self.stored
            || self.required > self.stored
            || self.unique > self.enabled
            || self.unique > 256
            || self.stored
                < region
                    .stored_noise()
                    .checked_add(region.stored_spatial())
                    .ok_or_else(limit)?
            || self.enabled
                < region
                    .visible_noise()
                    .checked_add(region.visible_spatial())
                    .ok_or_else(limit)?
            || (self.unique == 0
                && (self.compile != 0 || self.retained != 0 || self.preparation != 0))
            || (self.unique != 0
                && (self.compile == 0 || self.retained < u64::from(self.unique) * 256))
            || self.preparation < self.retained
            || self.counter_bytes == 0
            || self.workload_digest != workload.digest()?
            || self.workload_digest != region.workload_digest()
            || self.evaluation
                != self
                    .retained
                    .checked_add(region.workspace_bytes())
                    .and_then(|n| n.checked_add(self.counter_bytes))
                    .ok_or_else(limit)?
            || self.work
                != region.work().checked_add(WorkComponents {
                    base: self.compile,
                    noise: 0,
                })?
        {
            return Err(bad());
        }
        if self.workspace_peak()? > workload.limits().bytes {
            return Err(limit());
        }
        Ok(())
    }
    pub fn encode(
        self,
        region: SourceNoiseRegionContext,
        workload: Workload,
    ) -> Result<[u8; PROGRAM_BYTES]> {
        self.validate_region(region, workload)?;
        self.validate_current_abi()?;
        if region.source_bytes() == 0 {
            return Err(bad());
        }
        Ok(self.wire())
    }
    pub(super) fn wire(self) -> [u8; PROGRAM_BYTES] {
        let mut b = [0; PROGRAM_BYTES];
        write_header(&mut b);
        for (at, n) in [
            (8, self.required),
            (12, self.stored),
            (16, self.enabled),
            (20, self.unique),
        ] {
            b[at..at + 4].copy_from_slice(&n.to_be_bytes());
        }
        for (at, n) in [
            (24, self.compile),
            (32, self.retained),
            (40, self.preparation),
            (48, self.evaluation),
            (56, self.work.base),
            (64, self.work.noise),
            (72, self.work.checked_total().expect("validated components")),
            (80, self.counter_bytes),
            (88, self.metadata_bytes),
        ] {
            b[at..at + 8].copy_from_slice(&n.to_be_bytes());
        }
        for (at, hash) in [
            (96, self.required_digest),
            (128, self.resolution_digest),
            (160, self.workload_digest),
            (192, self.capability_digest),
        ] {
            b[at..at + 32].copy_from_slice(&hash);
        }
        b
    }
    pub(super) fn from_wire(bytes: &[u8]) -> Result<Self> {
        header(bytes, PROGRAM_BYTES)?;
        if bytes[224..] != [0; 32] {
            return Err(bad());
        }
        let work = WorkComponents {
            base: n64(bytes, 56),
            noise: n64(bytes, 64),
        };
        if n64(bytes, 72) != work.checked_total()? {
            return Err(bad());
        }
        Ok(Self {
            required: n32(bytes, 8),
            stored: n32(bytes, 12),
            enabled: n32(bytes, 16),
            unique: n32(bytes, 20),
            compile: n64(bytes, 24),
            retained: n64(bytes, 32),
            preparation: n64(bytes, 40),
            evaluation: n64(bytes, 48),
            work,
            counter_bytes: n64(bytes, 80),
            metadata_bytes: n64(bytes, 88),
            required_digest: bytes[96..128].try_into().unwrap(),
            resolution_digest: bytes[128..160].try_into().unwrap(),
            workload_digest: bytes[160..192].try_into().unwrap(),
            capability_digest: bytes[192..224].try_into().unwrap(),
        })
    }
    pub fn decode(
        bytes: &[u8],
        region: SourceNoiseRegionContext,
        workload: Workload,
    ) -> Result<Self> {
        let value = Self::from_wire(bytes)?;
        value.validate_region(region, workload)?;
        value.validate_current_abi()?;
        if region.source_bytes() == 0 {
            return Err(bad());
        }
        Ok(value)
    }
}
