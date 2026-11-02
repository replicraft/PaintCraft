//! Exact Source8 frames. from_frames validates metadata, never adopted data.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumericalFrames8 {
    pub count: usize,
    pub required: [u8; 32],
    pub resolution: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact8 {
    pub count: usize,
    pub required: [u8; 32],
    pub resolution: [u8; 32],
    pub own: LayerMasks7,
    pub source_coverage: [u8; 32],
    pub mask_inventory: [u8; 32],
    pub layer_blend: [u8; 32],
    pub capability: [u8; 32],
}
fn prefix(
    h: &mut Sha256,
    count: usize,
    required: [u8; 32],
    resolution: [u8; 32],
    admission: Admission8,
    registrations: (AppearanceRegistration, BlendRegistration),
    abi: CoverageAbi,
) -> Result<()> {
    h.update(8u16.to_be_bytes());
    h.update(4u16.to_be_bytes());
    h.update(u32::try_from(count).map_err(|_| limit())?.to_be_bytes());
    h.update(required);
    h.update(resolution);
    h.update(admission.digest()?);
    mask_append(registrations.0, h)?;
    registrations.1.append(h)?;
    text(h, abi.id())?;
    Ok(())
}
impl Fact8 {
    pub fn capture(
        required: &Requirements8,
        admission: Admission8,
        inventory: Inventory7,
        layer_blend: [u8; 32],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        admission.validate_requirements(required)?;
        let (r, s) = required.requirements().digests(check)?;
        Self::from_frames(
            NumericalFrames8 {
                count: required.requirements().len(),
                required: r,
                resolution: s,
            },
            admission,
            required.mask_appearance(),
            required.blend_appearance(),
            inventory,
            layer_blend,
        )
    }
    pub fn from_frames(
        numerical: NumericalFrames8,
        admission: Admission8,
        mask: AppearanceRegistration,
        blend: BlendRegistration,
        inventory: Inventory7,
        layer_blend: [u8; 32],
    ) -> Result<Self> {
        let NumericalFrames8 {
            count,
            required,
            resolution,
        } = numerical;
        inventory.validate()?;
        if count > ALGORITHM_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-capabilities/v8\0");
        prefix(
            &mut h,
            count,
            required,
            resolution,
            admission,
            (mask, blend),
            inventory.own.abi,
        )?;
        let own = inventory.own;
        h.update(own.stored.to_be_bytes());
        h.update(own.enabled.to_be_bytes());
        for n in [own.bands, own.runs, own.decode_upper_bytes] {
            h.update(n.to_be_bytes());
        }
        h.update(own.identity);
        h.update(inventory.source_coverage);
        h.update(inventory.mask_inventory);
        h.update(layer_blend);
        Ok(Self {
            count,
            required,
            resolution,
            own,
            source_coverage: inventory.source_coverage,
            mask_inventory: inventory.mask_inventory,
            layer_blend,
            capability: h.finalize().into(),
        })
    }
    pub fn validate_binding(
        self,
        admission: Admission8,
        mask: AppearanceRegistration,
        blend: BlendRegistration,
    ) -> Result<()> {
        // This path receives no full occurrence summary; validate only the fixed
        // own facts and exact frame. Adoption must prove actual inventories.
        self.own.validate()?;
        if self.count > ALGORITHM_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-capabilities/v8\0");
        prefix(
            &mut h,
            self.count,
            self.required,
            self.resolution,
            admission,
            (mask, blend),
            self.own.abi,
        )?;
        h.update(self.own.stored.to_be_bytes());
        h.update(self.own.enabled.to_be_bytes());
        for n in [self.own.bands, self.own.runs, self.own.decode_upper_bytes] {
            h.update(n.to_be_bytes());
        }
        h.update(self.own.identity);
        h.update(self.source_coverage);
        h.update(self.mask_inventory);
        h.update(self.layer_blend);
        let actual: [u8; 32] = h.finalize().into();
        if actual != self.capability {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Union8 {
    pub count: usize,
    pub required: [u8; 32],
    pub resolution: [u8; 32],
    pub source_coverage: [u8; 32],
    pub layer_blend: [u8; 32],
    pub capability: [u8; 32],
}
impl Union8 {
    pub fn capture(
        required: &Requirements8,
        admission: Admission8,
        source_coverage: [u8; 32],
        layer_blend: [u8; 32],
        abi: CoverageAbi,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        admission.validate_requirements(required)?;
        let (r, s) = required.requirements().digests(check)?;
        Self::from_frames(
            NumericalFrames8 {
                count: required.requirements().len(),
                required: r,
                resolution: s,
            },
            admission,
            required.mask_appearance(),
            required.blend_appearance(),
            source_coverage,
            layer_blend,
            abi,
        )
    }
    pub fn from_frames(
        numerical: NumericalFrames8,
        admission: Admission8,
        mask: AppearanceRegistration,
        blend: BlendRegistration,
        source_coverage: [u8; 32],
        layer_blend: [u8; 32],
        abi: CoverageAbi,
    ) -> Result<Self> {
        let NumericalFrames8 {
            count,
            required,
            resolution,
        } = numerical;
        if count > UNION_MAX {
            return Err(limit());
        }
        let mut h = Sha256::new();
        h.update(b"paintcraft.worker-capability-union/v8\0");
        prefix(
            &mut h,
            count,
            required,
            resolution,
            admission,
            (mask, blend),
            abi,
        )?;
        h.update(source_coverage);
        h.update(layer_blend);
        Ok(Self {
            count,
            required,
            resolution,
            source_coverage,
            layer_blend,
            capability: h.finalize().into(),
        })
    }
    pub fn validate_binding(
        self,
        admission: Admission8,
        mask: AppearanceRegistration,
        blend: BlendRegistration,
        abi: CoverageAbi,
    ) -> Result<()> {
        if self
            != Self::from_frames(
                NumericalFrames8 {
                    count: self.count,
                    required: self.required,
                    resolution: self.resolution,
                },
                admission,
                mask,
                blend,
                self.source_coverage,
                self.layer_blend,
                abi,
            )?
        {
            return Err(invalid());
        }
        Ok(())
    }
}
