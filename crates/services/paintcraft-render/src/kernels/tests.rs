use super::*;

// Independent u64 straight-alpha oracle, including original hidden RGB policy.
fn reference(s: [u8; 4], d: [u8; 4], opacity: u8) -> [u8; 4] {
    let a = (u64::from(s[3]) * u64::from(opacity) + 127) / 255;
    if a == 0 {
        return d;
    }
    let b = u64::from(d[3]);
    let total = a * 255 + b * (255 - a);
    let mut out = [0; 4];
    for c in 0..3 {
        out[c] = ((u64::from(s[c]) * a * 255 + u64::from(d[c]) * b * (255 - a) + total / 2) / total)
            as u8;
    }
    out[3] = ((total + 127) / 255) as u8;
    out
}

#[cfg(all(feature = "simd", not(feature = "simd-u16")))]
#[test]
fn vector_round255_matches_integer_division_throughout_the_proven_domain() {
    for n in 0..=65025 {
        assert_eq!(round255(u32x4::splat(n)).to_array(), [(n + 127) / 255; 4]);
    }
}

#[test]
fn every_source_alpha_opacity_and_channel_pair_matches_straight_alpha_oracle() {
    // Exercise every channel pair with every effective alpha in opaque output.
    for alpha in 0..=255u8 {
        for source in 0..=255u8 {
            for dest in 0..=255u8 {
                let s = [source, 255 - source, dest, alpha].repeat(4);
                let mut d = [dest, source, 255 - dest, 255].repeat(4);
                let expected =
                    reference(s[..4].try_into().unwrap(), d[..4].try_into().unwrap(), 255);
                dense(&mut d, &s, 255);
                assert_eq!(d, expected.repeat(4));
            }
        }
    }
    for opacity in 0..=255u8 {
        for alpha in 0..=255u8 {
            let s = [0, 127, 255, alpha].repeat(4);
            let mut d = [255, 128, 0, 255].repeat(4);
            let expected = reference([0, 127, 255, alpha], [255, 128, 0, 255], opacity);
            dense(&mut d, &s, opacity);
            assert_eq!(d, expected.repeat(4));
        }
    }
}

#[test]
fn mixed_destination_alpha_unaligned_slices_and_vector_tails_match_oracle() {
    for count in [0, 1, 3, 4, 5, 7, 127, 128, 129, 255, 256, 257] {
        for offset in 0..16 {
            for opacity in [0, 1, 127, 128, 254, 255] {
                for dest_alpha in 0..=255u8 {
                    let mut s = vec![0; offset + count * 4];
                    let mut d = vec![43; offset + count * 4 + 9];
                    for i in 0..count {
                        s[offset + i * 4..][..4].copy_from_slice(&[
                            i as u8,
                            (i * 19) as u8,
                            255 - i as u8,
                            (i * 37) as u8,
                        ]);
                        d[offset + i * 4 + 3] = if i % 8 < 4 { 255 } else { dest_alpha };
                    }
                    let mut expected = d.clone();
                    for (i, source) in s[offset..].as_chunks::<4>().0.iter().enumerate() {
                        let at = offset + i * 4;
                        let prior = expected[at..at + 4].try_into().unwrap();
                        expected[at..at + 4].copy_from_slice(&reference(*source, prior, opacity));
                    }
                    dense(&mut d[offset..offset + count * 4], &s[offset..], opacity);
                    assert_eq!(
                        d, expected,
                        "count={count} offset={offset} opacity={opacity}"
                    );
                }
            }
        }
    }
}
