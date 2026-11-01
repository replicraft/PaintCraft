use super::*;
/// Trusted endpoint policy, never a wire selector. No Default or production
/// constructor is exposed: Root must select ONE tuple before Wasm activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlicePolicy {
    edge: i64,
    base: u64,
    noise: u64,
}
impl SlicePolicy {
    pub fn for_qualification(edge: u32, base: u64, noise: u64) -> Result<Self> {
        if !(1..=64).contains(&edge)
            || base == 0
            || base > 8_388_608
            || noise == 0
            || noise > 134_217_728
        {
            return Err(resource("Source6 slice policy exceeds its finite domain"));
        }
        WorkComponents { base, noise }.checked_total()?;
        Ok(Self {
            edge: i64::from(edge),
            base,
            noise,
        })
    }
    /// Frozen protocol6/class1 QUALIFICATION tuple, not ordinary UI activation.
    pub const fn selected_qualification() -> Self {
        Self {
            edge: 64,
            base: 8_388_608,
            noise: 134_217_728,
        }
    }
    pub fn native_edge(self) -> i64 {
        self.edge
    }
    pub fn base_limit(self) -> u64 {
        self.base
    }
    pub fn noise_limit(self) -> u64 {
        self.noise
    }
    pub(crate) fn check(self, work: WorkComponents) -> Result<()> {
        work.checked_total()?;
        if work.base > self.base || work.noise > self.noise {
            return Err(resource(
                "Source6 native step exceeds its scoped slice policy",
            ));
        }
        Ok(())
    }
}
