//! Pure fixed-policy metadata; this module creates no counter or budget realm.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode7 {
    Base = 0,
    Noise = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Admission7 {
    pub mode: Mode7,
    pub workspace_bytes: u64,
    pub base_work: u64,
    pub noise_work: u64,
}
impl Admission7 {
    pub const WORKSPACE_MAX: u64 = 64 << 20;
    pub const BASE_MAX: u64 = 268_435_456;
    pub const NOISE_MAX: u64 = 1 << 35;
    pub const EDGE: u16 = 64;
    pub const BASE_SLICE: u64 = 8_388_608;
    pub const NOISE_SLICE: u64 = 134_217_728;
    pub fn total(self) -> Result<u64> {
        self.base_work
            .checked_add(self.noise_work)
            .ok_or_else(limit)
    }
    pub fn validate(self) -> Result<()> {
        if self.workspace_bytes > Self::WORKSPACE_MAX
            || self.base_work > Self::BASE_MAX
            || self.noise_work > Self::NOISE_MAX
            || (self.mode == Mode7::Base && self.noise_work != 0)
        {
            return Err(limit());
        }
        self.total()?;
        Ok(())
    }
    /// Noise may be pre-admitted for a prospective operation on Noise-free source.
    pub fn validate_requirements(self, required: &Requirements7) -> Result<()> {
        self.validate()?;
        if required.requires_noise()? && self.mode != Mode7::Noise {
            return Err(unsupported());
        }
        Ok(())
    }
    pub const fn noise_slice(self) -> u64 {
        match self.mode {
            Mode7::Base => 0,
            Mode7::Noise => Self::NOISE_SLICE,
        }
    }
    pub fn digest(self) -> Result<[u8; 32]> {
        self.validate()?;
        let mut h = Sha256::new();
        h.update(b"paintcraft.source7-admission/v1\0");
        h.update([self.mode as u8]);
        for n in [
            self.workspace_bytes,
            self.base_work,
            self.noise_work,
            self.total()?,
        ] {
            h.update(n.to_be_bytes());
        }
        h.update(1u16.to_be_bytes());
        h.update(Self::EDGE.to_be_bytes());
        h.update(Self::BASE_SLICE.to_be_bytes());
        h.update(self.noise_slice().to_be_bytes());
        Ok(h.finalize().into())
    }
}
