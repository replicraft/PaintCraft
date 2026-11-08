//! Strict borrowed ICC recipe using only the caller's original reservation.
use super::*;
use paintcraft_core::types::resources::{BudgetAccount, BudgetReservation, BudgetScope};
mod borrowed_wire;
const CONTROL: u64 = 4096;
const CHECK: u64 = 512;
// Each canonical helper zeroes192 bytes and writes at most187 field bytes.
// This is distinct from hashing its result or comparing the final header.
const CANONICAL_WORK: u64 = 384;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservedSourceMetadataLayout {
    pub inner_bytes: u64,
    pub inner_allocation_upper: u64,
    pub lease_control_allocation_upper: u64,
    pub constructor_control_bytes: u64,
}
fn extent() -> Error {
    resource("reserved source metadata extent")
}
fn arc_inner() -> Result<u64> {
    let align = std::mem::align_of::<Inner>().max(std::mem::align_of::<usize>());
    let offset = (2 * std::mem::size_of::<usize>())
        .checked_next_multiple_of(std::mem::align_of::<Inner>())
        .ok_or_else(extent)?;
    offset
        .checked_add(std::mem::size_of::<Inner>())
        .and_then(|n| n.checked_next_multiple_of(align))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(extent)
}
impl SourceMetadata {
    /// Defining-type full allocation uppers; no reservation or grant is created.
    pub fn reserved_layout_quote() -> Result<ReservedSourceMetadataLayout> {
        let named = (std::mem::size_of::<ReservedSourceMetadataReader<'static>>()
            + 2 * std::mem::size_of::<SourceMetadataRecipe<'static, 'static, 'static>>()
            + 2 * std::mem::size_of::<Sha256>()
            + 2 * WIRE_HEADER_BYTES
            + std::mem::size_of::<Inner>()
            + std::mem::size_of::<Vec<u8>>()
            + 2 * std::mem::size_of::<IccHeader>()
            + 2 * std::mem::size_of::<SourceDescriptor>()
            + 16 * std::mem::size_of::<u64>()) as u64;
        if named > CONTROL {
            return Err(extent());
        }
        Ok(ReservedSourceMetadataLayout {
            inner_bytes: std::mem::size_of::<Inner>() as u64,
            inner_allocation_upper: arc_inner()?,
            lease_control_allocation_upper: BudgetLease::control_allocation_quote()?,
            constructor_control_bytes: CONTROL,
        })
    }
    /// Existing retention policy, distinct from full Inner/Charge Arc guards in M.
    pub fn reserved_retained_quote(profile_bytes: u64) -> Result<u64> {
        profile_bytes
            .checked_add(WIRE_HEADER_BYTES as u64)
            .filter(|n| *n <= BYTES_MAX)
            .and_then(|n| n.checked_add(std::mem::size_of::<Inner>() as u64 + 32))
            .ok_or_else(extent)
    }
}
/// Bound once to original ICC/control/M declarations and raw trusted callbacks.
/// No scope, lease/account or owned byte extraction is available.
/// Caller MUST retain original M through returned SourceMetadata and every alias.
/// This primitive does not provide the later closed Source8 source carrier.
pub struct ReservedSourceMetadataReader<'a> {
    reservation: &'a BudgetReservation,
    _control_scope: &'a BudgetScope,
    guard: &'a BudgetAccount,
    limit: u64,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    _control: BudgetAccount,
}
impl<'a> ReservedSourceMetadataReader<'a> {
    pub fn new(
        reservation: &'a BudgetReservation,
        control_scope: &'a BudgetScope,
        guard: &'a BudgetAccount,
        limit: u64,
        check: &'a dyn Fn() -> Result<()>,
        spend: &'a dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        let q = SourceMetadata::reserved_layout_quote()?;
        if guard.bytes()
            < q.inner_allocation_upper
                .checked_add(q.lease_control_allocation_upper)
                .ok_or_else(extent)?
        {
            return Err(resource("source metadata full tail guard missing"));
        }
        let mut control = control_scope.account();
        control.resize(CONTROL)?;
        let reader = Self {
            reservation,
            _control_scope: control_scope,
            guard,
            limit,
            check,
            spend,
            _control: control,
        };
        reader.step(64)?;
        Ok(reader)
    }
    fn step(&self, n: u64) -> Result<()> {
        (self.spend)(n.checked_add(CHECK).ok_or_else(extent)?)?;
        (self.check)()
    }
    pub fn inspect<'r, 'w>(
        &'r self,
        header: &'w [u8],
        profile: &'w [u8],
    ) -> Result<SourceMetadataRecipe<'r, 'w, 'a>> {
        self.step(WIRE_HEADER_BYTES as u64)?;
        let descriptor = borrowed_wire::parse(header, profile)?;
        let p = profile.len() as u64;
        if p.checked_add(WIRE_HEADER_BYTES as u64)
            .is_none_or(|n| n > self.limit.min(BYTES_MAX))
        {
            return Err(resource("Source metadata allowance"));
        }
        self.step(64)?;
        descriptor.validate()?;
        let present = header[109] == 1;
        let profile_header = if present {
            self.step(132)?;
            let tags = profile
                .get(128..132)
                .ok_or_else(|| invalid("Truncated ICC header"))?;
            let work = u64::from(u32::from_be_bytes(tags.try_into().unwrap()))
                .checked_mul(12)
                .ok_or_else(extent)?;
            self.step(work)?;
            Some(inspect_icc(profile, &|| self.step(0))?)
        } else {
            None
        };
        let profile_sha256 = if present {
            self.step(p)?;
            let mut hash = Sha256::new();
            for chunk in profile.chunks(65536) {
                self.step(0)?;
                hash.update(chunk);
            }
            hash.finalize().into()
        } else {
            [0; 32]
        };
        self.step(CANONICAL_WORK)?;
        let zero_header =
            borrowed_wire::canonical(descriptor, present, profile.len(), profile_sha256, [0; 32]);
        self.step(
            (WIRE_HEADER_BYTES as u64)
                .checked_add(p)
                .ok_or_else(extent)?,
        )?;
        let mut hash = Sha256::new();
        hash.update(zero_header);
        for chunk in profile.chunks(65536) {
            self.step(0)?;
            hash.update(chunk);
        }
        let digest = hash.finalize().into();
        self.step(CANONICAL_WORK)?;
        let canonical =
            borrowed_wire::canonical(descriptor, present, profile.len(), profile_sha256, digest);
        self.step(WIRE_HEADER_BYTES as u64)?;
        if canonical.as_slice() != header {
            return Err(invalid("Source metadata digest/content differs"));
        }
        Ok(SourceMetadataRecipe {
            reader: self,
            header,
            profile,
            descriptor,
            profile_header,
            profile_sha256,
            digest,
        })
    }
}
/// Privately validated borrowed bytes tied to their original reader/callbacks.
/// No public fields, Clone or caller-set validity flag. Materialize has no args.
/// Returned recipe controls live in the caller ABI; containing state must fund
/// retained collections of recipes. The fixed slot covers this narrow operation.
/// ```compile_fail
/// use paintcraft_document::source_metadata::SourceMetadataRecipe;
/// use paintcraft_core::types::resources::BudgetReservation;
/// fn replace(recipe: &SourceMetadataRecipe<'_, '_, '_>, other: &BudgetReservation) {
///     let _ = recipe.materialize(other);
/// }
/// ```
/// ```compile_fail
/// use paintcraft_document::source_metadata::SourceMetadataRecipe;
/// fn detach(recipe: SourceMetadataRecipe<'_, '_, '_>) -> Vec<u8> {
///     recipe.into_vec()
/// }
/// ```
pub struct SourceMetadataRecipe<'r, 'w, 'a> {
    reader: &'r ReservedSourceMetadataReader<'a>,
    header: &'w [u8],
    profile: &'w [u8],
    descriptor: SourceDescriptor,
    profile_header: Option<IccHeader>,
    profile_sha256: [u8; 32],
    digest: [u8; 32],
}
impl SourceMetadataRecipe<'_, '_, '_> {
    pub fn profile_len(&self) -> usize {
        self.profile.len()
    }
    pub fn materialize(&self) -> Result<SourceMetadata> {
        let r = self.reader;
        r.step(64)?;
        let q = SourceMetadata::reserved_layout_quote()?;
        if r.guard.bytes()
            < q.inner_allocation_upper
                .checked_add(q.lease_control_allocation_upper)
                .ok_or_else(extent)?
        {
            return Err(resource("source metadata full tail guard missing"));
        }
        let charged = SourceMetadata::reserved_retained_quote(self.profile.len() as u64)?;
        // Original reservation ONLY; its fresh Charge Arc is already guarded in M.
        let lease = r.reservation.reserve(charged)?;
        r.step(self.profile.len() as u64)?;
        let mut profile = Vec::new();
        profile
            .try_reserve_exact(self.profile.len())
            .map_err(|_| extent())?;
        if profile.capacity() != self.profile.len() {
            return Err(extent());
        }
        profile.resize(self.profile.len(), 0);
        r.step(self.profile.len() as u64)?;
        for (source, destination) in self.profile.chunks(65536).zip(profile.chunks_mut(65536)) {
            r.step(0)?;
            destination.copy_from_slice(source);
        }
        r.step(64)?;
        Ok(SourceMetadata(Arc::new(Inner {
            descriptor: self.descriptor,
            profile,
            profile_header: self.profile_header,
            profile_sha256: self.profile_sha256,
            digest: self.digest,
            charged_bytes: charged,
            _lease: lease,
            _capacity_lease: None,
        })))
    }
}
impl std::fmt::Debug for ReservedSourceMetadataReader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReservedSourceMetadataReader")
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for SourceMetadataRecipe<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceMetadataRecipe")
            .field("header_bytes", &self.header.len())
            .field("profile_bytes", &self.profile.len())
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
mod tests;
