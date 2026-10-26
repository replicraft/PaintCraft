use super::*;
use std::cell::Cell;
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/dds-mip-fixtures/");
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{ROOT}{name}.dds")).unwrap()
}
fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
#[test]
fn independent_pixels_and_complete_layout_for_each_qualified_encoding() {
    let expected = std::fs::read(format!("{ROOT}top.rgba")).unwrap();
    for family in ["legacy", "dx10"] {
        let mut encodings = vec!["rgba", "bgra", "bc1", "bc2", "bc3"];
        if family == "dx10" {
            encodings.extend(["bc1-srgb", "bc2-srgb", "bc3-srgb"]);
        }
        for encoding in encodings {
            let bytes = fixture(&format!("{family}-{encoding}"));
            let result = decode_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
            assert_eq!(result.rgba, expected, "{family}-{encoding}");
            assert_eq!(result.rgba.len(), 7 * 5 * 4);
            assert_eq!(result.losses(), [OTHER_MIPS_LOSS]);
            assert_eq!(
                result.info.image.transfer,
                if family == "legacy" {
                    Transfer::Unspecified
                } else if encoding == "rgba" || encoding == "bgra" || encoding.ends_with("-srgb") {
                    Transfer::Srgb
                } else {
                    Transfer::Linear
                }
            );
            let levels = &result.info.levels;
            assert_eq!(
                levels
                    .iter()
                    .map(|m| (m.width, m.height))
                    .collect::<Vec<_>>(),
                [(7, 5), (3, 2), (1, 1)]
            );
            assert_eq!(
                levels[0].byte_offset,
                if family == "dx10" { 148 } else { 128 }
            );
            for pair in levels.windows(2) {
                assert_eq!(pair[0].byte_offset + pair[0].byte_len, pair[1].byte_offset);
            }
            assert_eq!(
                levels[2].byte_offset + levels[2].byte_len,
                bytes.len() as u64
            );
            assert!(inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
            assert!(decode_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
        }
    }
}
#[test]
fn only_top_surface_is_read_and_lower_encoded_data_cannot_affect_pixels() {
    let mut bytes = fixture("dx10-rgba");
    let check = || Ok(());
    let control = Control::new(&check);
    let (mut decoder, plan) = prepare_profile(&bytes, PIXELS_MAX, &control, true).unwrap();
    let rgba = read_rgba(&mut decoder, plan.image, &control).unwrap();
    let end = (plan.levels[0].byte_offset + plan.levels[0].byte_len) as usize;
    let reader = decoder.into_reader();
    assert_eq!(reader.remaining.as_ptr(), bytes[end..].as_ptr());
    assert_eq!(reader.remaining, &bytes[end..]);
    bytes[end..].fill(0xa5);
    assert_eq!(
        decode_top_mip_checked(&bytes, PIXELS_MAX, &check)
            .unwrap()
            .rgba,
        rgba
    );
}
#[test]
fn full_tail_and_budget_validation_precede_pixel_decode() {
    let bytes = fixture("dx10-bc3");
    let mut full_chain_linear_size = bytes.clone();
    put(&mut full_chain_linear_size, 20, 96);
    assert!(inspect_top_mip_checked(&full_chain_linear_size, PIXELS_MAX, &|| Ok(())).is_err());
    for end in 0..bytes.len() {
        assert!(
            inspect_top_mip_checked(&bytes[..end], PIXELS_MAX, &|| Ok(())).is_err(),
            "prefix {end}"
        );
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(decode_top_mip_checked(&extra, PIXELS_MAX, &|| Ok(())).is_err());
    assert!(decode_top_mip_checked(&bytes, 34, &|| Ok(())).is_err());
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() >= 4 {
            Err(resource("cancelled fixture"))
        } else {
            Ok(())
        }
    };
    assert!(decode_top_mip_checked(&bytes, PIXELS_MAX, &check).is_err());
}
#[test]
fn inconsistent_counts_geometry_flags_and_non_2d_resources_are_refused() {
    for (offset, value) in [
        (28, 4),
        (28, u32::MAX),
        (28, 0),
        (8, 0x100f),
        (108, 0x1000),
        (108, 0x401000),
        (108, 0x400008),
        (8, 0x21006),
        (112, 0x200),
        (128, 97),
        (132, 4),
        (136, 4),
        (140, 2),
        (144, 2),
        (20, 141),
        (16, 65_537),
        (12, 0),
    ] {
        let mut bytes = fixture("dx10-rgba");
        put(&mut bytes, offset, value);
        assert!(
            inspect_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err(),
            "offset {offset} value {value}"
        );
    }
}
#[test]
fn partial_chain_and_one_level_remain_distinct() {
    let mut bytes = fixture("dx10-rgba");
    bytes.truncate(148 + 140 + 24);
    put(&mut bytes, 28, 2);
    assert_eq!(
        inspect_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(()))
            .unwrap()
            .levels
            .len(),
        2
    );
    bytes.truncate(148 + 140);
    put(&mut bytes, 28, 1);
    put(&mut bytes, 8, 0x100f);
    put(&mut bytes, 108, 0x1000);
    let top = decode_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
    assert!(top.losses().is_empty());
    assert_eq!(
        decode_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap().rgba,
        top.rgba
    );
    put(&mut bytes, 108, 0x401008);
    assert!(inspect_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
}

#[test]
fn longest_skinny_chain_uses_geometry_bound_instead_of_an_arbitrary_level_cap() {
    let mut bytes = fixture("legacy-bc1");
    bytes.truncate(128);
    put(&mut bytes, 12, 1);
    put(&mut bytes, 16, 65_536);
    put(&mut bytes, 20, 131_072);
    put(&mut bytes, 28, 17);
    let mut width = 65_536u32;
    for _ in 0..17 {
        let blocks = width.div_ceil(4) as usize;
        for _ in 0..blocks {
            bytes.extend_from_slice(&[0, 0xf8, 0, 0, 0, 0, 0, 0]);
        }
        width = (width / 2).max(1);
    }
    let plan = inspect_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
    assert_eq!(plan.levels.len(), 17);
    assert_eq!((plan.levels[16].width, plan.levels[16].height), (1, 1));
    put(&mut bytes, 28, 18);
    bytes.extend_from_slice(&[0; 8]);
    assert!(inspect_top_mip_checked(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
}
