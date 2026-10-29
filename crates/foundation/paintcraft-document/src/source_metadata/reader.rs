//! Direct retained ICC stream capture: admit before inflation or allocation.
use super::*;
use std::io::Read;

impl SourceMetadata {
    /// Read exactly the already-inspected original profile into its final
    /// immutable storage. Expected identity binds the source inspector's bytes.
    /// Library reads are not a hard cancellation-latency or RSS contract.
    pub fn capture_profile_reader(
        descriptor: SourceDescriptor,
        profile_bytes: u64,
        expected_sha256: [u8; 32],
        mut reader: impl Read,
        limit: u64,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        descriptor.validate()?;
        if profile_bytes < 132 {
            return Err(invalid("Inspected ICC extent is shorter than its header"));
        }
        let total = profile_bytes
            .checked_add(WIRE_HEADER_BYTES as u64)
            .filter(|n| *n <= limit.min(BYTES_MAX))
            .ok_or_else(|| resource("Source metadata allowance"))?;
        let charge = total
            .checked_add(std::mem::size_of::<Inner>() as u64 + 32)
            .ok_or_else(|| resource("Source metadata storage extent"))?;
        let lease = budget.reserve(charge)?;
        let length = usize::try_from(profile_bytes)
            .map_err(|_| resource("Retained ICC address allowance"))?;
        let mut profile = Vec::new();
        profile
            .try_reserve_exact(length)
            .map_err(|_| resource("Cannot allocate retained ICC"))?;
        let excess = profile.capacity() as u64 - profile_bytes;
        let capacity_lease = (excess != 0).then(|| budget.reserve(excess)).transpose()?;
        let mut hash = Sha256::new();
        while profile.len() < length {
            check()?;
            let start = profile.len();
            profile.resize((start + 65536).min(length), 0);
            reader
                .read_exact(&mut profile[start..])
                .map_err(|_| invalid("Truncated retained ICC stream"))?;
            hash.update(&profile[start..]);
        }
        check()?;
        let mut extra = [0; 1];
        if reader
            .read(&mut extra)
            .map_err(|_| invalid("Malformed retained ICC stream"))?
            != 0
        {
            return Err(invalid("Retained ICC stream exceeds its inspected extent"));
        }
        let profile_sha256: [u8; 32] = hash.finalize().into();
        if profile_sha256 != expected_sha256 {
            return Err(invalid(
                "Retained ICC differs from inspected source identity",
            ));
        }
        let header = inspect_icc(&profile, check)?;
        finish(
            Inner {
                descriptor,
                profile,
                profile_header: Some(header),
                profile_sha256,
                digest: [0; 32],
                charged_bytes: charge + excess,
                _lease: lease,
                _capacity_lease: capacity_lease,
            },
            check,
        )
    }
}

pub(super) fn finish(mut inner: Inner, check: &dyn Fn() -> Result<()>) -> Result<SourceMetadata> {
    inner.digest.fill(0);
    let mut hash = Sha256::new();
    hash.update(wire::header(&inner));
    for chunk in inner.profile.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    inner.digest = hash.finalize().into();
    check()?;
    Ok(SourceMetadata(Arc::new(inner)))
}
