//! Streaming canonical frames. Input order is checked; callers pre-admit sorting.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageAbi {
    Wasm32V1,
    Native64V1,
}
impl CoverageAbi {
    pub fn current() -> Result<Self> {
        if cfg!(target_arch = "wasm32") {
            Ok(Self::Wasm32V1)
        } else if cfg!(target_pointer_width = "64") {
            Ok(Self::Native64V1)
        } else {
            Err(unsupported())
        }
    }
    pub const fn id(self) -> &'static str {
        match self {
            Self::Wasm32V1 => "wasm32-v1",
            Self::Native64V1 => "native64-v1",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoverageKey {
    pub layer: [u8; 16],
    pub kind: u8,
    pub filter: [u8; 16],
}
impl CoverageKey {
    pub fn validate(self) -> Result<()> {
        if self.kind > 1 || (self.kind == 1 && self.filter != [0; 16]) {
            return Err(invalid());
        }
        Ok(())
    }
    fn append(self, h: &mut Sha256) {
        h.update(self.layer);
        h.update([self.kind]);
        h.update(self.filter);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverageRow {
    pub key: CoverageKey,
    pub enabled: bool,
    pub domain: [i64; 4],
    pub digest: [u8; 32],
    pub bands: u64,
    pub runs: u64,
    pub decode_upper_bytes: u64,
}
impl CoverageRow {
    fn validate(self) -> Result<()> {
        self.key.validate()?;
        if self.domain[2] <= self.domain[0]
            || self.domain[3] <= self.domain[1]
            || self.bands > 262_144
            || self.runs > 262_144
            || self.bands > self.runs
            || ((self.bands == 0) != (self.runs == 0))
        {
            return Err(invalid());
        }
        Ok(())
    }
    fn tail(self, h: &mut Sha256) {
        for n in self.domain {
            h.update(n.to_be_bytes());
        }
        h.update(self.digest);
        for n in [self.bands, self.runs, self.decode_upper_bytes] {
            h.update(n.to_be_bytes());
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerMasks7 {
    pub stored: u32,
    pub enabled: u32,
    pub bands: u64,
    pub runs: u64,
    pub decode_upper_bytes: u64,
    pub identity: [u8; 32],
    pub abi: CoverageAbi,
}
impl LayerMasks7 {
    pub fn validate(self) -> Result<()> {
        if self.stored > OWN_MAX
            || self.enabled > self.stored
            || self.bands > 262_144
            || self.runs > 262_144
            || self.bands > self.runs
            || ((self.bands == 0) != (self.runs == 0))
        {
            return Err(limit());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inventory7 {
    pub own: LayerMasks7,
    pub occurrences: u32,
    pub bands: u64,
    pub runs: u64,
    pub decode_upper_bytes: u64,
    pub source_coverage: [u8; 32],
    pub mask_inventory: [u8; 32],
}
impl Inventory7 {
    pub fn filter_coverage(self) -> Result<crate::pcnd::CoverageFacts> {
        self.validate()?;
        Ok(crate::pcnd::CoverageFacts {
            masks: u64::from(
                self.occurrences
                    .checked_sub(self.own.stored)
                    .ok_or_else(limit)?,
            ),
            bands: self.bands.checked_sub(self.own.bands).ok_or_else(limit)?,
            runs: self.runs.checked_sub(self.own.runs).ok_or_else(limit)?,
            decode_bytes: self
                .decode_upper_bytes
                .checked_sub(self.own.decode_upper_bytes)
                .ok_or_else(limit)?,
        })
    }
    pub fn validate(self) -> Result<()> {
        self.own.validate()?;
        if self.occurrences > OCCURRENCE_MAX
            || self.occurrences < self.own.stored
            || self.bands > 262_144
            || self.runs > 262_144
            || self.bands > self.runs
            || ((self.bands == 0) != (self.runs == 0))
            || self.bands < self.own.bands
            || self.runs < self.own.runs
            || self.decode_upper_bytes < self.own.decode_upper_bytes
        {
            return Err(limit());
        }
        Ok(())
    }
}
#[derive(Debug)]
struct Ordered {
    expected: u32,
    seen: u32,
    last: Option<CoverageKey>,
}
impl Ordered {
    fn new(expected: u32) -> Result<Self> {
        if expected > OCCURRENCE_MAX {
            return Err(limit());
        }
        Ok(Self {
            expected,
            seen: 0,
            last: None,
        })
    }
    fn push(&mut self, key: CoverageKey) -> Result<()> {
        key.validate()?;
        if self.seen >= self.expected || self.last.is_some_and(|last| last >= key) {
            return Err(invalid());
        }
        self.seen += 1;
        self.last = Some(key);
        Ok(())
    }
    fn finish(self) -> Result<()> {
        if self.seen != self.expected {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct CoverageHasher {
    h: Sha256,
    order: Ordered,
}
impl CoverageHasher {
    pub fn new(abi: CoverageAbi, count: u32) -> Result<Self> {
        let order = Ordered::new(count)?;
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-coverage-bounds/v1\0");
        text(&mut h, abi.id())?;
        h.update(1u16.to_be_bytes());
        h.update(count.to_be_bytes());
        Ok(Self { h, order })
    }
    pub fn push(&mut self, row: CoverageRow) -> Result<()> {
        row.validate()?;
        self.order.push(row.key)?;
        row.key.append(&mut self.h);
        self.h.update([u8::from(row.enabled)]);
        row.tail(&mut self.h);
        Ok(())
    }
    pub fn finish(self) -> Result<[u8; 32]> {
        self.order.finish()?;
        Ok(self.h.finalize().into())
    }
}
#[derive(Debug)]
pub struct InventoryHasher {
    h: Sha256,
    order: Ordered,
}
impl InventoryHasher {
    pub fn new(count: u32) -> Result<Self> {
        let order = Ordered::new(count)?;
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-mask-inventory/v1\0");
        h.update(count.to_be_bytes());
        Ok(Self { h, order })
    }
    pub fn push(
        &mut self,
        row: CoverageRow,
        instance_enabled: bool,
        source: CoverageKey,
    ) -> Result<()> {
        row.validate()?;
        source.validate()?;
        self.order.push(row.key)?;
        if source.kind != row.key.kind
            || (row.key.kind == 1 && (!instance_enabled || source != row.key))
        {
            return Err(invalid());
        }
        row.key.append(&mut self.h);
        self.h
            .update([u8::from(instance_enabled), u8::from(row.enabled)]);
        row.tail(&mut self.h);
        source.append(&mut self.h);
        Ok(())
    }
    pub fn finish(self) -> Result<[u8; 32]> {
        self.order.finish()?;
        Ok(self.h.finalize().into())
    }
}
#[derive(Debug)]
pub struct IdentityHasher {
    h: Sha256,
    expected: u32,
    seen: u32,
    last: Option<[u8; 16]>,
}
impl IdentityHasher {
    pub fn new(count: u32) -> Result<Self> {
        if count > OWN_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-layer-masks/v1\0");
        h.update(count.to_be_bytes());
        Ok(Self {
            h,
            expected: count,
            seen: 0,
            last: None,
        })
    }
    pub fn push(&mut self, layer: [u8; 16], enabled: bool, digest: [u8; 32]) -> Result<()> {
        if self.seen >= self.expected || self.last.is_some_and(|last| last >= layer) {
            return Err(invalid());
        }
        self.seen += 1;
        self.last = Some(layer);
        self.h.update(layer);
        self.h.update([u8::from(enabled)]);
        self.h.update(digest);
        Ok(())
    }
    pub fn finish(self) -> Result<[u8; 32]> {
        if self.seen != self.expected {
            return Err(invalid());
        }
        Ok(self.h.finalize().into())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact7 {
    pub count: usize,
    pub required: [u8; 32],
    pub resolution: [u8; 32],
    pub own: LayerMasks7,
    pub source_coverage: [u8; 32],
    pub mask_inventory: [u8; 32],
    pub capability: [u8; 32],
}
fn prefix(
    h: &mut Sha256,
    required: [u8; 32],
    resolution: [u8; 32],
    admission: Admission7,
    appearance: AppearanceRegistration,
) -> Result<()> {
    h.update(3u16.to_be_bytes());
    h.update(required);
    h.update(resolution);
    h.update(admission.digest()?);
    appearance.append(h)
}
impl Fact7 {
    /// Recompute only framing against resolved endpoint metadata. This does NOT
    /// attest declared coverage inventories, numerical resolution, or authority.
    pub fn validate_binding(
        self,
        admission: Admission7,
        appearance: AppearanceRegistration,
    ) -> Result<()> {
        self.own.validate()?;
        if self.count > ALGORITHM_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-capabilities/v3\0");
        prefix(
            &mut h,
            self.required,
            self.resolution,
            admission,
            appearance,
        )?;
        h.update(self.own.stored.to_be_bytes());
        h.update(self.own.enabled.to_be_bytes());
        for n in [self.own.bands, self.own.runs, self.own.decode_upper_bytes] {
            h.update(n.to_be_bytes());
        }
        h.update(self.own.identity);
        h.update(self.source_coverage);
        h.update(self.mask_inventory);
        let digest: [u8; 32] = h.finalize().into();
        if digest != self.capability {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn capture(
        required: &Requirements7,
        admission: Admission7,
        inventory: Inventory7,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        inventory.validate()?;
        admission.validate_requirements(required)?;
        if required.requirements().len() > ALGORITHM_MAX {
            return Err(limit());
        }
        let (r, s) = required.requirements().digests(check)?;
        Self::from_frames(
            required.requirements().len(),
            r,
            s,
            admission,
            required.appearance(),
            inventory,
        )
    }
    /// Opaque numerical digests already resolved by the caller; also used by
    /// independent framing literals. This is metadata, never execution authority.
    pub fn from_frames(
        count: usize,
        required: [u8; 32],
        resolution: [u8; 32],
        admission: Admission7,
        appearance: AppearanceRegistration,
        inventory: Inventory7,
    ) -> Result<Self> {
        inventory.validate()?;
        if count > ALGORITHM_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-capabilities/v3\0");
        prefix(&mut h, required, resolution, admission, appearance)?;
        let own = inventory.own;
        h.update(own.stored.to_be_bytes());
        h.update(own.enabled.to_be_bytes());
        for n in [own.bands, own.runs, own.decode_upper_bytes] {
            h.update(n.to_be_bytes());
        }
        h.update(own.identity);
        h.update(inventory.source_coverage);
        h.update(inventory.mask_inventory);
        Ok(Self {
            count,
            required,
            resolution,
            own,
            source_coverage: inventory.source_coverage,
            mask_inventory: inventory.mask_inventory,
            capability: h.finalize().into(),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Union7 {
    pub count: usize,
    pub required: [u8; 32],
    pub resolution: [u8; 32],
    pub source_coverage: [u8; 32],
    pub capability: [u8; 32],
}
impl Union7 {
    pub fn validate_binding(
        self,
        admission: Admission7,
        appearance: AppearanceRegistration,
    ) -> Result<()> {
        if self
            != Self::from_frames(
                self.count,
                self.required,
                self.resolution,
                admission,
                appearance,
                self.source_coverage,
            )?
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub fn capture(
        required: &Requirements7,
        admission: Admission7,
        source_coverage: [u8; 32],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        admission.validate_requirements(required)?;
        let (r, s) = required.requirements().digests(check)?;
        Self::from_frames(
            required.requirements().len(),
            r,
            s,
            admission,
            required.appearance(),
            source_coverage,
        )
    }
    pub fn from_frames(
        count: usize,
        required: [u8; 32],
        resolution: [u8; 32],
        admission: Admission7,
        appearance: AppearanceRegistration,
        source_coverage: [u8; 32],
    ) -> Result<Self> {
        if count > UNION_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-capability-union/v1\0");
        prefix(&mut h, required, resolution, admission, appearance)?;
        h.update(source_coverage);
        Ok(Self {
            count,
            required,
            resolution,
            source_coverage,
            capability: h.finalize().into(),
        })
    }
}
