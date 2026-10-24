//! Exact dense RGBA8 composition with an opaque-destination specialization.
use paintcraft_core::types::Rgba;

#[cfg(all(feature = "simd", not(feature = "simd-u16")))]
use wide::u32x4;

#[cfg(feature = "simd-u16")]
mod lanes;

pub(super) fn dense(dst: &mut [u8], src: &[u8], opacity: u8) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len() % 4, 0);
    let (dest, dest_tail) = dst.as_chunks_mut::<16>();
    let (source, source_tail) = src.as_chunks::<16>();
    #[cfg(feature = "simd-u16")]
    let context = cfg!(any(
        all(target_arch = "aarch64", target_feature = "neon"),
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2"
        ),
        all(target_arch = "wasm32", target_feature = "simd128"),
    ))
    .then(|| lanes::Opaque::new(opacity));
    for (d, s) in dest.iter_mut().zip(source) {
        // The same branch overhead applies to scalar and SIMD builds.
        if d[3] & d[7] & d[11] & d[15] == 255 {
            #[cfg(not(feature = "simd-u16"))]
            opaque(d, s, opacity);
            #[cfg(feature = "simd-u16")]
            match &context {
                Some(context) => context.blend(d, s),
                None => opaque(d, s, opacity),
            }
        } else {
            scalar(d, s, opacity);
        }
    }
    scalar(dest_tail, source_tail, opacity);
}

fn scalar(dst: &mut [u8], src: &[u8], opacity: u8) {
    for (d, s) in dst
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.as_chunks::<4>().0)
    {
        let color = Rgba(*s).with_opacity(opacity);
        match color.a() {
            0 => {}
            255 => *d = color.0,
            _ => *d = color.source_over(Rgba(*d)).0,
        }
    }
}

#[cfg(any(not(feature = "simd"), feature = "simd-u16"))]
fn opaque(dst: &mut [u8; 16], src: &[u8; 16], opacity: u8) {
    for (d, s) in dst
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.as_chunks::<4>().0)
    {
        let alpha = (u32::from(s[3]) * u32::from(opacity) + 127) / 255;
        for c in 0..3 {
            d[c] = ((u32::from(s[c]) * alpha + u32::from(d[c]) * (255 - alpha) + 127) / 255) as u8;
        }
    }
}

#[cfg(all(feature = "simd", not(feature = "simd-u16")))]
fn opaque(dst: &mut [u8; 16], src: &[u8; 16], opacity: u8) {
    // Little-endian numeric packing is explicit, independent of host byte order.
    let load = |bytes: &[u8; 16]| {
        u32x4::new(std::array::from_fn(|i| {
            u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
        }))
    };
    let s = load(src);
    let d = load(dst);
    let mask = u32x4::splat(255);
    let alpha = round255((s >> 24) * u32x4::splat(u32::from(opacity)));
    let inverse = mask - alpha;
    #[cfg(feature = "simd-channels")]
    let channel =
        |shift: u32| round255(((s >> shift) & mask) * alpha + ((d >> shift) & mask) * inverse);
    #[cfg(feature = "simd-channels")]
    let result: u32x4 = channel(0) | (channel(8) << 8) | (channel(16) << 16) | (mask << 24);
    #[cfg(not(feature = "simd-channels"))]
    let result: u32x4 = {
        // Red and blue occupy independent 16-bit fields. Their weighted sums are
        // <= 65025 each; rounding stays <= 65407, so no field carries into its
        // neighbor and every packed intermediate fits in u32.
        let rb_mask = u32x4::splat(0x00ff_00ff);
        let rb = (s & rb_mask) * alpha + (d & rb_mask) * inverse;
        let t = rb + u32x4::splat(0x0080_0080);
        let rb = ((t + ((t >> 8) & rb_mask)) >> 8) & rb_mask;
        let green = round255(((s >> 8) & mask) * alpha + ((d >> 8) & mask) * inverse);
        rb | (green << 8) | (mask << 24)
    };
    for (d, value) in dst.as_chunks_mut::<4>().0.iter_mut().zip(result.to_array()) {
        *d = value.to_le_bytes();
    }
}

/// Round n/255 half up for 0 <= n <= 65025. All intermediates fit in u32.
#[cfg(all(feature = "simd", not(feature = "simd-u16")))]
fn round255(n: u32x4) -> u32x4 {
    let t = n + u32x4::splat(128);
    (t + (t >> 8)) >> 8
}

#[cfg(test)]
mod tests;
