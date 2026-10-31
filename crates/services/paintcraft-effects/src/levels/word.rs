//! Sealed word operations; no unsafe casts or arbitrary external implementation.
use std::fmt::Debug;
pub(super) trait Word: Debug {
    type Limb: Copy + Debug + Ord;
    const ZERO: Self::Limb;
    const ONE: Self::Limb;
    const BITS: u32;
    const CHUNK: usize;
    const REMAINDER_CHARGE: u64;
    fn from_u16(value: u16) -> Self::Limb;
    fn checked_mul(a: Self::Limb, b: Self::Limb) -> Option<Self::Limb>;
    fn mul_add(
        a: Self::Limb,
        b: Self::Limb,
        old: Self::Limb,
        carry: Self::Limb,
    ) -> (Self::Limb, Self::Limb);
}
#[derive(Debug)]
pub(super) struct U32;
impl Word for U32 {
    type Limb = u32;
    const ZERO: u32 = 0;
    const ONE: u32 = 1;
    const BITS: u32 = 32;
    const CHUNK: usize = 64;
    const REMAINDER_CHARGE: u64 = 0;
    fn from_u16(value: u16) -> u32 {
        u32::from(value)
    }
    fn checked_mul(a: u32, b: u32) -> Option<u32> {
        a.checked_mul(b)
    }
    fn mul_add(a: u32, b: u32, old: u32, carry: u32) -> (u32, u32) {
        // (2^32-1)^2 +2*(2^32-1) == u64::MAX.
        let n = u64::from(a) * u64::from(b) + u64::from(old) + u64::from(carry);
        (n as u32, (n >> 32) as u32)
    }
}
#[derive(Debug)]
pub(super) struct U64;
impl Word for U64 {
    type Limb = u64;
    const ZERO: u64 = 0;
    const ONE: u64 = 1;
    const BITS: u32 = 64;
    const CHUNK: usize = 32;
    const REMAINDER_CHARGE: u64 = 64;
    fn from_u16(value: u16) -> u64 {
        u64::from(value)
    }
    fn checked_mul(a: u64, b: u64) -> Option<u64> {
        a.checked_mul(b)
    }
    fn mul_add(a: u64, b: u64, old: u64, carry: u64) -> (u64, u64) {
        // (2^64-1)^2 +2*(2^64-1) == u128::MAX.
        let n = u128::from(a) * u128::from(b) + u128::from(old) + u128::from(carry);
        (n as u64, (n >> 64) as u64)
    }
}
