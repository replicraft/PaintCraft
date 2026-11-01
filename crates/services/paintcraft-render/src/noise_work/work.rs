use super::*;
/// Distinct modeled work; Noise credit never buys existing-profile work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkComponents {
    pub base: u64,
    pub noise: u64,
}
impl WorkComponents {
    pub fn checked_total(self) -> Result<u64> {
        self.base.checked_add(self.noise).ok_or_else(limit)
    }
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        let value = Self {
            base: self.base.checked_add(rhs.base).ok_or_else(limit)?,
            noise: self.noise.checked_add(rhs.noise).ok_or_else(limit)?,
        };
        value.checked_total()?;
        Ok(value)
    }
    pub fn checked_mul(self, n: u64) -> Result<Self> {
        let value = Self {
            base: self.base.checked_mul(n).ok_or_else(limit)?,
            noise: self.noise.checked_mul(n).ok_or_else(limit)?,
        };
        value.checked_total()?;
        Ok(value)
    }
}
