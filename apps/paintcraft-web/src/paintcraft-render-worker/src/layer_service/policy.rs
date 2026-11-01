use super::*;
/// Compiled protocol7 revision1 tuple; never a user-selectable quantum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SlicePolicy {
    noise: bool,
}
impl SlicePolicy {
    pub fn for_mode(noise: bool) -> Self {
        Self { noise }
    }
    pub fn native_edge(self) -> i64 {
        64
    }
    pub fn base_limit(self) -> u64 {
        8_388_608
    }
    pub fn noise_limit(self) -> u64 {
        if self.noise { 134_217_728 } else { 0 }
    }
    pub fn check(self, work: WorkComponents) -> Result<()> {
        work.checked_total()?;
        if work.base > self.base_limit() || work.noise > self.noise_limit() {
            return Err(resource("Source7 native slice exceeds immutable policy"));
        }
        Ok(())
    }
}
