//! Fixed control metadata only; never a variable-length encoded-source parser.
use super::{controls::CompiledControls8, policy::*};
use paintcraft_core::types::Result;
pub(super) const E: usize = 0;
pub(super) const P: usize = 1;
pub(super) const M: usize = 2;
pub(super) const A: usize = 3;
pub(super) const I: usize = 4;
pub(super) const C: usize = 5;
pub(super) const Q: usize = 6;
pub(super) const K: usize = 7;
pub(super) const W: usize = 8;
pub(super) const F: usize = 9;
pub(super) const O: usize = 10;
pub(super) const H: usize = 11;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Binding8 {
    pub owner: [u8; 16],
    pub public_state: [u8; 16],
    pub public_epoch: u64,
    pub document: [u8; 16],
    pub source_state: [u8; 16],
    pub draft: [u8; 16],
    pub source_sha: [u8; 32],
    pub profile_sha: [u8; 32],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Open8 {
    pub binding: Binding8,
    pub roles: [u64; 12],
    pub bytes: u64,
    pub base_work: u64,
    pub noise_work: u64,
    pub paired: u64,
    pub canvas: [u32; 2],
    pub flags: u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Command8 {
    pub open: Open8,
    pub token: u64,
    pub generation: u64,
    pub visible: [u32; 4],
    pub level: u32,
    pub ordinal: u32,
    pub request: u32,
    pub kind: u8,
}
fn u32_at(w: &[u8; WINDOW], i: usize) -> u32 {
    u32::from_le_bytes(w[i..i + 4].try_into().expect("fixed compiled offset"))
}
fn u64_at(w: &[u8; WINDOW], i: usize) -> u64 {
    u64::from_le_bytes(w[i..i + 8].try_into().expect("fixed compiled offset"))
}
fn bytes<const N: usize>(w: &[u8; WINDOW], i: usize) -> [u8; N] {
    w[i..i + N].try_into().expect("fixed compiled offset")
}
impl Command8 {
    pub fn decode(w: &[u8; WINDOW]) -> Result<Self> {
        if &w[..4] != b"PC8W"
            || w[4..6] != [1, 0]
            || !(1..=5).contains(&w[6])
            || w[7] & !7 != 0
            || u32_at(w, 8) == 0
            || u32_at(w, 12) != 0
            || w[344..].iter().any(|b| *b != 0)
        {
            return Err(bad());
        }
        Ok(Self {
            open: Open8 {
                binding: Binding8 {
                    owner: bytes(w, 16),
                    public_state: bytes(w, 32),
                    public_epoch: u64_at(w, 48),
                    document: bytes(w, 56),
                    source_state: bytes(w, 72),
                    draft: bytes(w, 88),
                    source_sha: bytes(w, 104),
                    profile_sha: bytes(w, 136),
                },
                roles: std::array::from_fn(|i| u64_at(w, 208 + 8 * i)),
                bytes: u64_at(w, 176),
                base_work: u64_at(w, 184),
                noise_work: u64_at(w, 192),
                paired: u64_at(w, 200),
                canvas: [u32_at(w, 168), u32_at(w, 172)],
                flags: w[7],
            },
            token: u64_at(w, 304),
            generation: u64_at(w, 312),
            visible: std::array::from_fn(|i| u32_at(w, 320 + 4 * i)),
            level: u32_at(w, 336),
            ordinal: u32_at(w, 340),
            request: u32_at(w, 8),
            kind: w[6],
        })
    }
    pub fn original(&self, original: &Open8) -> Result<()> {
        if self.open != *original {
            Err(stale())
        } else {
            Ok(())
        }
    }
    pub fn open(&self, controls: CompiledControls8, profile: [u8; 32]) -> Result<()> {
        if self.kind != 1
            || self.token != 0
            || self.generation != 0
            || self.visible != [0; 4]
            || self.level != 0
            || self.ordinal != 0
        {
            return Err(bad());
        }
        self.open.validate(controls, profile)
    }
}
impl Open8 {
    pub fn noise(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn stored_noise(&self) -> bool {
        self.flags & 2 != 0
    }
    pub fn validate(&self, controls: CompiledControls8, profile: [u8; 32]) -> Result<()> {
        if self.flags & 4 == 0
            || (!self.noise() && (self.stored_noise() || self.noise_work != 0 || self.paired != 0))
            || self.bytes == 0
            || self.bytes > JOB
            || self.base_work == 0
            || self.base_work > BASE
            || self.noise_work > NOISE
            || self.binding.profile_sha != profile
            || self.canvas.iter().any(|n| !(1..=1048576).contains(n))
        {
            return Err(bad());
        }
        let sum = self
            .roles
            .iter()
            .try_fold(self.paired, |a, n| a.checked_add(*n).ok_or_else(bound))?;
        if sum > JOB
            || self.roles[E] < self.bytes
            || self.roles[H] != controls.total()?
            || !self.roles[Q].is_multiple_of(4)
            || self.roles[F] < 16384
            || self.roles[O] < 116
            || self.roles[W] == 0
        {
            return Err(bound());
        }
        let read = crate::source8::source8_reader_control_bytes()?;
        let material = paintcraft_formats::pcnd::portable_source8_materializer_control_bytes()?;
        let plan = paintcraft_formats::pcnd::portable_source8_control_bytes();
        let records = plan.plan.checked_add(56).ok_or_else(bound)?;
        let census = plan
            .scan
            .checked_add(material.retained_layout)
            .ok_or_else(bound)?;
        let probe = paintcraft_formats::pcnd::layer_blend_appearance::transient_quote(1)?;
        let min_p = records.checked_add(census.max(probe)).ok_or_else(bound)?;
        if self.roles[C] < read.entry.checked_add(read.assembly).ok_or_else(bound)?
            || self.roles[K] < read.retained_carriers
            || self.roles[A]
                < material
                    .coverage
                    .checked_add(material.document_stream)
                    .ok_or_else(bound)?
            || self.roles[P] < min_p
        {
            return Err(bound());
        }
        let side = if self.noise() {
            paintcraft_render::noise_work::NoiseAdmission::control_quote()?.counter_bytes
        } else {
            0
        };
        if self.paired != side || self.roles[W] < side {
            return Err(bound());
        }
        Ok(())
    }
}
