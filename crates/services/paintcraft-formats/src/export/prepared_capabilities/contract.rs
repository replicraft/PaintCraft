//! Execution metadata negotiation is separate from generic descriptor decoding.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum RegistryContract {
    Legacy1 = 1,
    ScopedNoise2 = 2,
}
impl RegistryContract {
    pub const fn revision(self) -> u16 {
        self as u16
    }
}
impl TryFrom<u16> for RegistryContract {
    type Error = Error;
    fn try_from(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::Legacy1),
            2 => Ok(Self::ScopedNoise2),
            _ => Err(unsupported(
                "Prepared worker registry contract is unavailable",
            )),
        }
    }
}
impl Registration {
    /// Extra HELLO2 metadata. Existing required/resolution SHA framing is unchanged.
    /// This describes a priced class, not a counter, allowance or grant.
    pub fn workload_class(self) -> (&'static str, u32) {
        match self.execution {
            Execution::DirectNoise => ("noise-source-work-v1", 1),
            Execution::DirectPoint
            | Execution::RegionBox
            | Execution::PreparedLevels
            | Execution::PreparedExposure
            | Execution::PreparedCurves
            | Execution::DirectVibrance
            | Execution::DirectHueSaturation => ("base-work-v1", 1),
        }
    }
    pub fn validate_contract(self, contract: RegistryContract) -> Result<()> {
        match (contract, self.execution) {
            (RegistryContract::Legacy1, Execution::DirectNoise) => Err(unsupported(
                "Noise requires scoped worker registry contract 2",
            )),
            (RegistryContract::ScopedNoise2, Execution::DirectNoise)
            | (
                RegistryContract::Legacy1 | RegistryContract::ScopedNoise2,
                Execution::DirectPoint
                | Execution::RegionBox
                | Execution::PreparedLevels
                | Execution::PreparedExposure
                | Execution::PreparedCurves
                | Execution::DirectVibrance
                | Execution::DirectHueSaturation,
            ) => Ok(()),
        }
    }
}
/// Resolve a strict known pair AND the endpoint's explicit execution contract.
/// General PCND decoding/resolve alone deliberately remains broader metadata support.
pub fn resolve_for(
    contract: RegistryContract,
    profile: &str,
    codec: &str,
) -> Result<&'static Registration> {
    let entry = resolve(profile, codec)?;
    entry.validate_contract(contract)?;
    Ok(entry)
}
impl Requirements {
    /// Validate the complete authenticated source/candidate/union before adoption.
    /// Includes disabled/default intentions, since this set contains no enable flags.
    pub fn validate_contract(
        &self,
        contract: RegistryContract,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        for entry in self.entries[..self.len].iter().flatten() {
            check()?;
            entry.validate_contract(contract)?;
        }
        Ok(())
    }
}
/// Routing only: inspect every stored typed descriptor, including disabled/default.
/// Strict resolution/decoder errors propagate; no unknown policy becomes false.
/// No scope/account/counter is created and no execution authority is returned.
pub fn requires_scoped_noise(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<bool> {
    let mut scoped = false;
    for layer in doc.layers.values() {
        for instance in layer.filters.iter() {
            check()?;
            let descriptor = instance.descriptor();
            let entry = resolve(descriptor.id(), "params-v1")?;
            let (bytes, length) = descriptor.semantic_parameter_bytes();
            if entry.decode(&bytes[..length])? != descriptor {
                return Err(invalid());
            }
            scoped |= match entry.execution {
                Execution::DirectNoise => true,
                Execution::DirectPoint
                | Execution::RegionBox
                | Execution::PreparedLevels
                | Execution::PreparedExposure
                | Execution::PreparedCurves
                | Execution::DirectVibrance
                | Execution::DirectHueSaturation => false,
            };
        }
    }
    Ok(scoped)
}
fn unsupported(message: &'static str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
#[cfg(test)]
mod tests;
