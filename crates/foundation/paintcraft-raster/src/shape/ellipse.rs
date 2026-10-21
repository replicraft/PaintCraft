//! Exact row bounds, including clipped shapes whose cross-products need 256 bits.
fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // The seed is approximate; both corrections are exact. n <= i64::MAX²,
    // so these products fit u128 even when the floating seed rounds upward.
    let mut x = (n as f64).sqrt() as u128;
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}
fn product(a: u128, b: u128) -> (u128, u128) {
    let mask = u128::from(u64::MAX);
    let (a0, a1, b0, b1) = (a & mask, a >> 64, b & mask, b >> 64);
    let (p00, p01, p10, p11) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
    let middle = (p00 >> 64) + (p01 & mask) + (p10 & mask);
    (
        p11 + (p01 >> 64) + (p10 >> 64) + (middle >> 64),
        (middle << 64) | (p00 & mask),
    )
}
pub(super) fn row(w: i64, h: i64, py: i64) -> Option<(i64, i64)> {
    if w <= 0 || h <= 0 || !(0..h).contains(&py) {
        return None;
    }
    let (w, h) = (w as u128, h as u128);
    let u = (2 * i128::from(py) + 1 - h as i128).unsigned_abs();
    let (w2, h2) = (w * w, h * h);
    let remaining = h2 - u * u;
    let m = if let Some(q) = w2.checked_mul(remaining) {
        isqrt(q / h2)
    } else {
        // m²*h² <= w²*(h²-u²), compared as exact (high,low) products.
        // At most 63 bisections for any valid i64 extent. No floating quotient.
        let rhs = product(w2, remaining);
        let (mut lo, mut hi) = (0, w);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if product(mid * mid, h2) <= rhs {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    };
    let (lo, hi) = ((w - m) / 2, (w - 1 + m) / 2);
    (lo <= hi).then_some((lo as i64, hi as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(a: u128, b: u128) -> (u128, u128) {
        let a: Vec<_> = (0..4).map(|i| (a >> (32 * i)) as u32).collect();
        let b: Vec<_> = (0..4).map(|i| (b >> (32 * i)) as u32).collect();
        let mut words = [0u32; 8];
        for (i, &a) in a.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in b.iter().enumerate() {
                let n = u64::from(a) * u64::from(b) + u64::from(words[i + j]) + carry;
                words[i + j] = n as u32;
                carry = n >> 32;
            }
            words[i + 4] = carry as u32;
        }
        let word = |start: usize| {
            (0usize..4).fold(0u128, |n, i| n | (u128::from(words[start + i]) << (32 * i)))
        };
        (word(4), word(0))
    }
    #[test]
    fn wide_products_match_independent_schoolbook_limbs() {
        let mut n = 0x123456789abcdefu128;
        for a in [0, 1, u64::MAX as u128, 1 << 127, u128::MAX] {
            for b in [0, 1, u64::MAX as u128, 1 << 127, u128::MAX] {
                assert_eq!(product(a, b), reference(a, b));
            }
        }
        for _ in 0..2048 {
            n = n
                .wrapping_mul(0xda942042e4dd58b5)
                .wrapping_add(0x9e3779b97f4a7c15);
            let a = n;
            n = n.rotate_left(61).wrapping_mul(0xd6e8feb86659fd93);
            assert_eq!(product(a, n), reference(a, n));
        }
    }
}
