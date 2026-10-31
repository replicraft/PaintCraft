//! Fixed nonnegative integers. No Copy/Clone or hidden full-width temporaries.
use super::*;
use std::cmp::Ordering;
pub(super) const LIMBS: usize = 320;
#[derive(Debug)]
pub(super) struct Integer {
    words: [u32; LIMBS],
    len: usize,
}
impl Integer {
    pub const fn zero() -> Self {
        Self {
            words: [0; LIMBS],
            len: 1,
        }
    }
    fn set(&mut self, value: u32, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge(1)?;
        self.words[0] = value;
        self.len = 1;
        Ok(())
    }
    pub fn pow(&mut self, base: u16, exponent: u16, budget: &mut Budget<'_>) -> Result<()> {
        // At most 31 grouping and 30 remainder steps, priced as one bounded
        // 64-unit block. Repeated word multiplication never squares past the
        // final requested exponent and never needs another large integer.
        budget.charge(64)?;
        self.set(1, budget)?;
        if exponent == 0 {
            return Ok(());
        }
        if base <= 1 {
            return self.set(u32::from(base), budget);
        }
        let mut factor = u32::from(base);
        let mut group = 1u16;
        while let Some(next) = factor.checked_mul(u32::from(base)) {
            factor = next;
            group += 1;
        }
        let mut remaining = exponent;
        while remaining >= group {
            self.mul_word(factor, budget)?;
            remaining -= group;
        }
        if remaining != 0 {
            let mut factor = 1u32;
            for _ in 0..remaining {
                factor *= u32::from(base);
            }
            self.mul_word(factor, budget)?;
        }
        Ok(())
    }
    fn mul_word(&mut self, factor: u32, budget: &mut Budget<'_>) -> Result<()> {
        let mut carry = 0u64;
        for words in self.words[..self.len].chunks_mut(64) {
            budget.charge(words.len() as u64)?;
            for word in words {
                let product = u64::from(*word) * u64::from(factor) + carry;
                *word = product as u32;
                carry = product >> 32;
            }
        }
        if carry != 0 {
            budget.charge(1)?;
            if self.len == LIMBS {
                return Err(resource("Levels integer capacity"));
            }
            self.words[self.len] = carry as u32;
            self.len += 1;
        }
        Ok(())
    }
    pub fn product(&mut self, a: &Self, b: &Self, budget: &mut Budget<'_>) -> Result<()> {
        let length = a
            .len
            .checked_add(b.len)
            .filter(|n| *n <= LIMBS)
            .ok_or_else(|| resource("Levels product capacity"))?;
        for words in self.words[..length].chunks_mut(64) {
            budget.charge(words.len() as u64)?;
            words.fill(0);
        }
        for i in 0..a.len {
            let mut carry = 0u64;
            for (chunk, words) in b.words[..b.len].chunks(64).enumerate() {
                budget.charge(words.len() as u64)?;
                for (offset, word) in words.iter().enumerate() {
                    let at = i + chunk * 64 + offset;
                    // (2^32-1)^2 + (2^32-1) + (2^32-1) <= u64::MAX.
                    let n = u64::from(a.words[i]) * u64::from(*word)
                        + u64::from(self.words[at])
                        + carry;
                    self.words[at] = n as u32;
                    carry = n >> 32;
                }
            }
            budget.charge(1)?;
            self.words[i + b.len] = carry as u32;
        }
        self.len = length;
        while self.len > 1 {
            budget.charge(1)?;
            if self.words[self.len - 1] != 0 {
                break;
            }
            self.len -= 1;
        }
        Ok(())
    }
    pub fn compare(&self, other: &Self, budget: &mut Budget<'_>) -> Result<Ordering> {
        budget.charge(1)?;
        if self.len != other.len {
            return Ok(self.len.cmp(&other.len));
        }
        for (a, b) in self.words[..self.len]
            .rchunks(64)
            .zip(other.words[..other.len].rchunks(64))
        {
            budget.charge(a.len() as u64)?;
            for (a, b) in a.iter().rev().zip(b.iter().rev()) {
                if a != b {
                    return Ok(a.cmp(b));
                }
            }
        }
        Ok(Ordering::Equal)
    }
}
pub(super) fn words(base: u16, exponent: u16) -> u64 {
    if base <= 1 {
        return 1;
    }
    (u64::from(16 - base.leading_zeros()) * u64::from(exponent))
        .div_ceil(32)
        .max(1)
}
pub(super) fn power_quote(base: u16, exponent: u16) -> u64 {
    // Grouped powers perform no more work than one word multiplication for
    // every exponent unit, including a possible carry word at each step.
    67 + u64::from(exponent) * (words(base, exponent) + 1)
}
pub(super) fn product_quote(a: u64, b: u64) -> u64 {
    a * b + 2 * (a + b) + a
}
