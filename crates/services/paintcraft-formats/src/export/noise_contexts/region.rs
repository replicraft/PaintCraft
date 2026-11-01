use super::*;
/// Per-layer source-local64 full-image quote, never Source6 display-patch totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceNoiseRegionContext {
    stored_spatial: u32,
    visible_spatial: u32,
    stored_noise: u32,
    visible_noise: u32,
    tiles: u64,
    workspace: u64,
    work: WorkComponents,
    payload: u64,
    source_bytes: u64,
    workload_digest: [u8; 32],
}
impl SourceNoiseRegionContext {
    /// base_payload must be the caller's authoritative canonical snapshot/output
    /// base, excluding appearance workspace and these new context controls.
    pub fn capture(
        doc: &Document,
        base_payload: u64,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        if doc.layers.len() > 8 {
            return Err(limit());
        }
        let required = Requirements::capture(doc, check)?;
        required.validate_contract(RegistryContract::ScopedNoise2, check)?;
        let mut value = Self {
            workload_digest: workload.digest()?,
            ..Self::default()
        };
        for layer in doc.layers.values() {
            check()?;
            for filter in layer.filters.iter() {
                check()?;
                if filter.descriptor().is_spatial() {
                    value.stored_spatial = value.stored_spatial.checked_add(1).ok_or_else(limit)?;
                }
                if matches!(filter.descriptor(), Descriptor::AddNoiseRgbV1(_)) {
                    value.stored_noise = value.stored_noise.checked_add(1).ok_or_else(limit)?;
                }
            }
        }
        if value.stored_noise == 0 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Scoped export profile requires stored Noise intent",
            ));
        }
        let canvas = doc.canvas_rect();
        if doc.canvas.0 == 0 || doc.canvas.1 == 0 || canvas.area() > 4_194_304 {
            return Err(limit());
        }
        for layer in doc
            .layers_in_order()
            .filter(|l| l.visible && l.opacity != 0)
        {
            check()?;
            let Some(area) = layer.extent().intersect(&canvas) else {
                continue;
            };
            let local = area
                .translated(
                    layer.origin.0.checked_neg().ok_or_else(limit)?,
                    layer.origin.1.checked_neg().ok_or_else(limit)?,
                )
                .ok_or_else(limit)?;
            for filter in layer.filters.iter().filter(|f| f.enabled()) {
                check()?;
                match filter.descriptor() {
                    Descriptor::BoxBlurV1(p) if p.radius() > 0 => {
                        value.visible_spatial =
                            value.visible_spatial.checked_add(1).ok_or_else(limit)?;
                    }
                    Descriptor::AddNoiseRgbV1(_) => {
                        value.visible_noise =
                            value.visible_noise.checked_add(1).ok_or_else(limit)?;
                    }
                    _ => {}
                }
            }
            let active = paintcraft_render::spatial::active(layer);
            // Trusted whole-local quote internally walks exactly64 tiles from
            // this clipped SOURCE-local origin, matching render_flat/spatial::visit.
            let q = noise_work::quote_layer(layer, local, check)?;
            value.tiles = value.tiles.checked_add(q.tiles).ok_or_else(limit)?;
            if active {
                value.workspace = value.workspace.max(q.bytes);
            }
            let mut cost = if active {
                q.work
            } else {
                q.work.checked_mul(2)?
            };
            cost.base = cost
                .base
                .checked_add(u64::try_from(area.area()).map_err(|_| limit())?)
                .ok_or_else(limit)?;
            value.work = value.work.checked_add(cost)?;
            workload.check(value.work)?;
        }
        value.payload = base_payload
            .checked_add(value.workspace)
            .ok_or_else(limit)?;
        value.validate(workload, PAYLOAD_MAX)?;
        check()?;
        Ok(value)
    }
    /// Rebind only the upstream snapshot base before actual source finalization.
    pub fn with_base_payload(mut self, base: u64, workload: Workload) -> Result<Self> {
        if self.source_bytes != 0 {
            return Err(bad());
        }
        self.payload = base.checked_add(self.workspace).ok_or_else(limit)?;
        self.validate(workload, PAYLOAD_MAX)?;
        Ok(self)
    }
    pub fn work(self) -> WorkComponents {
        self.work
    }
    pub fn workspace_bytes(self) -> u64 {
        self.workspace
    }
    pub fn payload_bytes(self) -> u64 {
        self.payload
    }
    pub fn source_bytes(self) -> u64 {
        self.source_bytes
    }
    pub fn stored_noise(self) -> u32 {
        self.stored_noise
    }
    pub fn stored_spatial(self) -> u32 {
        self.stored_spatial
    }
    pub fn visible_noise(self) -> u32 {
        self.visible_noise
    }
    pub fn visible_spatial(self) -> u32 {
        self.visible_spatial
    }
    pub fn workload_digest(self) -> [u8; 32] {
        self.workload_digest
    }
    pub fn finalize_source(
        mut self,
        source_bytes: usize,
        workload: Workload,
        cap: u64,
    ) -> Result<Self> {
        if self.source_bytes != 0 || source_bytes == 0 {
            return Err(bad());
        }
        self.source_bytes = u64::try_from(source_bytes).map_err(|_| limit())?;
        self.payload = self
            .payload
            .checked_add(self.source_bytes)
            .ok_or_else(limit)?;
        self.validate(workload, cap)?;
        Ok(self)
    }
    pub fn validate(self, workload: Workload, cap: u64) -> Result<()> {
        if self.workload_digest != workload.digest()? || cap > PAYLOAD_MAX {
            return Err(bad());
        }
        workload.check(self.work)?;
        if self.stored_spatial > STORED_MAX
            || self.stored_noise == 0
            || self.stored_noise > STORED_MAX
            || self
                .stored_spatial
                .checked_add(self.stored_noise)
                .ok_or_else(limit)?
                > STORED_MAX
            || self.visible_spatial > self.stored_spatial
            || self.visible_noise > self.stored_noise
            || self.workspace > workload.limits().bytes
            || self.source_bytes > 64 << 20
            || self.payload
                < self
                    .workspace
                    .checked_add(self.source_bytes)
                    .ok_or_else(limit)?
            || self.payload > cap
        {
            return Err(limit());
        }
        Ok(())
    }
    pub fn encode(self) -> Result<[u8; REGION_BYTES]> {
        if self.source_bytes == 0 {
            return Err(bad());
        }
        Ok(self.wire())
    }
    fn wire(self) -> [u8; REGION_BYTES] {
        let mut b = [0; REGION_BYTES];
        write_header(&mut b);
        for (at, n) in [
            (8, self.stored_spatial),
            (12, self.visible_spatial),
            (16, self.stored_noise),
            (20, self.visible_noise),
        ] {
            b[at..at + 4].copy_from_slice(&n.to_be_bytes());
        }
        for (at, n) in [
            (24, self.tiles),
            (32, self.workspace),
            (40, self.work.base),
            (48, self.work.noise),
            (56, self.work.checked_total().expect("validated components")),
            (64, self.payload),
            (72, self.source_bytes),
        ] {
            b[at..at + 8].copy_from_slice(&n.to_be_bytes());
        }
        b[80..112].copy_from_slice(&self.workload_digest);
        b
    }
    pub fn decode(bytes: &[u8], workload: Workload, cap: u64) -> Result<Self> {
        header(bytes, REGION_BYTES)?;
        if bytes[112..] != [0; 16] {
            return Err(bad());
        }
        let work = WorkComponents {
            base: n64(bytes, 40),
            noise: n64(bytes, 48),
        };
        if n64(bytes, 56) != work.checked_total()? {
            return Err(bad());
        }
        let value = Self {
            stored_spatial: n32(bytes, 8),
            visible_spatial: n32(bytes, 12),
            stored_noise: n32(bytes, 16),
            visible_noise: n32(bytes, 20),
            tiles: n64(bytes, 24),
            workspace: n64(bytes, 32),
            work,
            payload: n64(bytes, 64),
            source_bytes: n64(bytes, 72),
            workload_digest: bytes[80..112].try_into().unwrap(),
        };
        if value.source_bytes == 0 {
            return Err(bad());
        }
        value.validate(workload, cap)?;
        Ok(value)
    }
}
