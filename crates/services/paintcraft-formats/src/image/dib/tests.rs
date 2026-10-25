use super::*;
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn header(size: usize, w: i32, h: i32, bpp: u16, compression: u32) -> Vec<u8> {
    let mut bytes = vec![0; size];
    put(&mut bytes, 0, size as u32);
    put(&mut bytes, 4, w as u32);
    put(&mut bytes, 8, h as u32);
    bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&bpp.to_le_bytes());
    put(&mut bytes, 16, compression);
    bytes
}
fn read(bytes: &[u8], kind: DibKind) -> Result<Decoded> {
    decode_dib_with_pixel_limit(bytes, kind, 1024, &|| Ok(()))
}
const PIXELS: [[u8; 4]; 6] = [
    [17, 44, 99, 0],
    [255, 0, 7, 127],
    [90, 80, 70, 255],
    [11, 22, 33, 64],
    [5, 6, 7, 1],
    [255, 255, 255, 255],
];
fn rgb24(top: bool) -> Vec<u8> {
    let mut bytes = header(40, 3, if top { -2 } else { 2 }, 24, 0);
    for y in if top { [0, 1] } else { [1, 0] } {
        for x in 0..3 {
            let p = PIXELS[y * 3 + x];
            bytes.extend([p[2], p[1], p[0]]);
        }
        bytes.extend([0xa5; 3]);
    }
    bytes
}
fn rgba(size: usize, alpha: bool, compression: u32) -> Vec<u8> {
    let mut bytes = header(size, 3, -2, 32, compression);
    for (at, value) in [
        (40, 0xff0000),
        (44, 0xff00),
        (48, 0xff),
        (52, if alpha { 0xff000000 } else { 0 }),
        (56, 0x73524742),
    ] {
        put(&mut bytes, at, value);
    }
    for p in PIXELS {
        bytes.extend([p[2], p[1], p[0], p[3]]);
    }
    bytes
}
#[test]
fn padded_rgb24_top_down_and_bottom_up_match_independent_pixels() {
    for top in [false, true] {
        let result = read(&rgb24(top), DibKind::Dib).unwrap();
        assert_eq!((result.width, result.height), (3, 2));
        let expected: Vec<u8> = PIXELS
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        assert_eq!(result.rgba, expected);
        assert!(result.losses.is_empty());
    }
}
#[test]
fn v4_and_v5_explicit_masks_keep_first_pixels_alpha_and_hidden_rgb() {
    for size in [108, 124] {
        let bytes = rgba(size, true, 3);
        let result = read(
            &bytes,
            if size == 124 {
                DibKind::DibV5
            } else {
                DibKind::Dib
            },
        )
        .unwrap();
        assert_eq!(result.rgba, PIXELS.concat());
        assert_eq!(result.losses, ["dib-alpha-mode-unspecified"]);
    }
}
#[test]
fn undeclared_32_bit_alpha_is_opaque_with_explicit_loss() {
    for (size, alpha, compression) in [(108, false, 3), (124, false, 3), (124, true, 0)] {
        let result = read(&rgba(size, alpha, compression), DibKind::Dib).unwrap();
        let expected: Vec<u8> = PIXELS
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        assert_eq!(result.rgba, expected);
        assert!(result.losses.iter().any(|s| s == "dib-alpha-unspecified"));
    }
    let mut bytes = header(40, 1, 1, 32, 0);
    bytes.extend([30, 20, 10, 0]);
    assert_eq!(read(&bytes, DibKind::Dib).unwrap().rgba, [10, 20, 30, 255]);
}
#[test]
fn indexed_palettes_and_rgb555_565_use_exact_declared_layout() {
    for bpp in [1, 4, 8] {
        let mut bytes = header(40, 3, 1, bpp, 0);
        put(&mut bytes, 32, 2);
        bytes.extend([0, 0, 255, 0, 0, 255, 0, 0]);
        bytes.extend(match bpp {
            1 => [0x40, 0xff, 0xff, 0xff],
            4 => [0x01, 0x0f, 0xff, 0xff],
            _ => [0, 1, 0, 0xff],
        });
        let result = read(&bytes, DibKind::Dib).unwrap();
        assert_eq!(
            result.rgba,
            [255, 0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255]
        );
        assert_eq!(result.losses, ["palette-indices-expanded"]);
        let at = 48;
        bytes[at] = if bpp == 1 { 0xff } else { 0x30 };
        if bpp == 1 {
            put(&mut bytes, 32, 1);
            bytes.drain(44..48);
        }
        assert!(read(&bytes, DibKind::Dib).is_err());
    }
    for masks in [(0xf800u32, 0x7e0u32, 0x1fu32), (0x7c00, 0x3e0, 0x1f)] {
        let mut bytes = header(40, 3, 1, 16, 3);
        for mask in [masks.0, masks.1, masks.2] {
            bytes.extend(mask.to_le_bytes());
        }
        for value in [masks.0 as u16, masks.1 as u16, masks.2 as u16] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([0; 2]);
        assert_eq!(
            read(&bytes, DibKind::Dib).unwrap().rgba,
            [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]
        );
    }
}
#[test]
fn invalid_dimensions_masks_ranges_and_compressions_are_rejected_before_decoding() {
    let good = rgba(124, true, 3);
    for (at, value) in [
        (0, 56),
        (4, 0),
        (4, u32::MAX),
        (8, 0),
        (8, i32::MIN as u32),
        (12, 0),
        (14, 12),
        (16, 1),
        (16, 2),
        (16, 4),
        (16, 5),
        (20, 1),
        (40, 0),
        (52, 0x80000000),
        (112, 124),
        (116, u32::MAX),
    ] {
        let mut bytes = good.clone();
        put(&mut bytes, at, value);
        assert!(read(&bytes, DibKind::DibV5).is_err(), "{at}={value}");
    }
    for length in 0..good.len() {
        assert!(read(&good[..length], DibKind::DibV5).is_err());
    }
    assert!(read(&rgb24(true), DibKind::DibV5).is_err());
    assert_eq!(
        decode_dib_with_pixel_limit(&good, DibKind::DibV5, 5, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn profile_and_resolution_metadata_require_consent_and_never_resolve_paths() {
    let mut bytes = rgba(124, true, 3);
    let offset = bytes.len() as u32;
    let path = b"C:\\must-not-open.icc\0";
    put(&mut bytes, 56, 0x4c494e4b);
    put(&mut bytes, 112, offset);
    put(&mut bytes, 116, path.len() as u32);
    put(&mut bytes, 24, 3780);
    bytes.extend(path);
    let result = read(&bytes, DibKind::DibV5).unwrap();
    assert_eq!(result.rgba, PIXELS.concat());
    assert!(result.losses.iter().any(|v| v == "dib-metadata-ignored"));
    for (offset, length) in [(0, 0), (40, 1), (124, 1), (u32::MAX, 1)] {
        let mut bad = bytes.clone();
        put(&mut bad, 112, offset);
        put(&mut bad, 116, length);
        assert!(read(&bad, DibKind::DibV5).is_err());
    }
}
#[test]
fn every_observed_cancellation_point_discards_the_result() {
    use std::cell::Cell;
    let mut indexed = header(40, 3, 1, 8, 0);
    put(&mut indexed, 32, 2);
    indexed.extend([0, 0, 255, 0, 0, 255, 0, 0, 0, 1, 0, 0]);
    for (bytes, kind) in [
        (rgba(124, true, 3), DibKind::DibV5),
        (indexed, DibKind::Dib),
    ] {
        let calls = Cell::new(0);
        decode_dib_with_pixel_limit(&bytes, kind, 1024, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        for fail in 0..calls.get() {
            let count = Cell::new(0);
            let result = decode_dib_with_pixel_limit(&bytes, kind, 1024, &|| {
                let current = count.get();
                count.set(current + 1);
                if current == fail {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        }
    }
}
