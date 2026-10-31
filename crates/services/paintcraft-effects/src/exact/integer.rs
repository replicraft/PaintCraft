//! Fixed nonnegative integers. No Copy/Clone or hidden full-width temporaries.
use super::{Budget, Fault, Result, word::Word};
use std::cmp::Ordering;
#[derive(Debug)]
pub(crate) struct Integer<W: Word, const N: usize> {
    words: [W::Limb; N],
    len: usize,
}
impl<W: Word, const N: usize> Integer<W, N> {
    pub const fn zero() -> Self {
        Self {
            words: [W::ZERO; N],
            len: 1,
        }
    }
    fn set(&mut self, value: W::Limb, budget: &mut impl Budget) -> Result<()> {
        budget.charge(1)?;
        self.words[0] = value;
        self.len = 1;
        Ok(())
    }
    pub fn pow(&mut self, base: u16, exponent: u16, budget: &mut impl Budget) -> Result<()> {
        // At most BITS-1 grouping and BITS-2 remainder scalar steps.
        // U32 preserves its one64-unit block; U64 separately charges a second
        // block before a nonempty remainder. No final unnecessary squaring.
        budget.charge(64)?;
        self.set(W::ONE, budget)?;
        if exponent == 0 {
            return Ok(());
        }
        if base <= 1 {
            return self.set(W::from_u16(base), budget);
        }
        let mut factor = W::from_u16(base);
        let mut group = 1u16;
        while let Some(next) = W::checked_mul(factor, W::from_u16(base)) {
            factor = next;
            group += 1;
        }
        let mut remaining = exponent;
        while remaining >= group {
            self.mul_word(factor, budget)?;
            remaining -= group;
        }
        if remaining != 0 {
            if W::REMAINDER_CHARGE != 0 {
                budget.charge(W::REMAINDER_CHARGE)?;
            }
            let mut factor = W::ONE;
            for _ in 0..remaining {
                factor = W::checked_mul(factor, W::from_u16(base))
                    .ok_or_else(|| budget.resource(Fault::ScalarPower))?;
            }
            self.mul_word(factor, budget)?;
        }
        Ok(())
    }
    fn mul_word(&mut self, factor: W::Limb, budget: &mut impl Budget) -> Result<()> {
        let mut carry = W::ZERO;
        for words in self.words[..self.len].chunks_mut(W::CHUNK) {
            budget.charge(words.len() as u64)?;
            for word in words {
                (*word, carry) = W::mul_add(*word, factor, W::ZERO, carry);
            }
        }
        if carry != W::ZERO {
            budget.charge(1)?;
            if self.len == N {
                return Err(budget.resource(Fault::IntegerCapacity));
            }
            self.words[self.len] = carry;
            self.len += 1;
        }
        Ok(())
    }
    /// Multiply in place by exact 2^bits. One scalar factor plus the existing
    /// charged word multiply per group; all factors fit a sealed native limb.
    pub(crate) fn scale_pow2(&mut self, mut bits: u16, budget: &mut impl Budget) -> Result<()> {
        budget.charge(1)?;
        while bits != 0 {
            let count = u32::from(bits).min(W::BITS - 1);
            budget.charge(1)?;
            self.mul_word(W::power_of_two(count), budget)?;
            bits -= count as u16;
        }
        Ok(())
    }
    pub fn product(&mut self, a: &Self, b: &Self, budget: &mut impl Budget) -> Result<()> {
        let length = a
            .len
            .checked_add(b.len)
            .filter(|n| *n <= N)
            .ok_or_else(|| budget.resource(Fault::ProductCapacity))?;
        for words in self.words[..length].chunks_mut(W::CHUNK) {
            budget.charge(words.len() as u64)?;
            words.fill(W::ZERO);
        }
        for i in 0..a.len {
            let mut carry = W::ZERO;
            for (chunk, words) in b.words[..b.len].chunks(W::CHUNK).enumerate() {
                budget.charge(words.len() as u64)?;
                for (offset, word) in words.iter().enumerate() {
                    let at = i + chunk * W::CHUNK + offset;
                    // Each sealed word implementation proves its double-width
                    // sum fits, including existing destination and carry.
                    (self.words[at], carry) = W::mul_add(a.words[i], *word, self.words[at], carry);
                }
            }
            budget.charge(1)?;
            self.words[i + b.len] = carry;
        }
        self.len = length;
        while self.len > 1 {
            budget.charge(1)?;
            if self.words[self.len - 1] != W::ZERO {
                break;
            }
            self.len -= 1;
        }
        Ok(())
    }
    pub fn compare(&self, other: &Self, budget: &mut impl Budget) -> Result<Ordering> {
        budget.charge(1)?;
        if self.len != other.len {
            return Ok(self.len.cmp(&other.len));
        }
        for (a, b) in self.words[..self.len]
            .rchunks(W::CHUNK)
            .zip(other.words[..other.len].rchunks(W::CHUNK))
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
pub(crate) fn words<W: Word>(base: u16, exponent: u16) -> u64 {
    if base <= 1 {
        return 1;
    }
    (u64::from(16 - base.leading_zeros()) * u64::from(exponent))
        .div_ceil(u64::from(W::BITS))
        .max(1)
}
pub(crate) fn power_quote<W: Word>(base: u16, exponent: u16) -> u64 {
    // Grouped powers perform no more work than one word multiplication for
    // every exponent unit, including a possible carry word at each step.
    67 + W::REMAINDER_CHARGE + u64::from(exponent) * (words::<W>(base, exponent) + 1)
}
pub(crate) fn product_quote(a: u64, b: u64) -> u64 {
    a * b + 2 * (a + b) + a
}
