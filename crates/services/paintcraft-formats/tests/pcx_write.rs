use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    image::{self, ImageFormat},
    pcx,
};
use std::cell::Cell;

#[test]
fn rgb_odd_rows_rle_literals_and_long_runs_have_exact_pixels() {
    for width in [1, 3, 7, 63, 64, 65, 257] {
        let height = 3;
        let rgb: Vec<u8> = (0..width * height)
            .flat_map(|i| {
                if i % 5 == 0 {
                    [0xc1, 0xff, 0xc0]
                } else {
                    [17, 51, (i % 256) as u8]
                }
            })
            .collect();
        let bytes = pcx::encode_rgb(width, height, &rgb, (72, 144), &|| Ok(())).unwrap();
        assert_eq!(&bytes[..4], &[10, 5, 1, 8]);
        assert_eq!(bytes[65], 3);
        assert_eq!(
            u16::from_le_bytes(bytes[66..68].try_into().unwrap()) as u32,
            width + (width & 1)
        );
        assert_eq!(&bytes[12..16], &[72, 0, 144, 0]);
        let decoded = image::decode(&bytes).unwrap();
        let expected: Vec<u8> = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        assert_eq!((decoded.width, decoded.height), (width, height));
        assert_eq!(decoded.rgba, expected);
    }
}

#[test]
fn indexed_output_preserves_duplicate_palette_entries_and_original_index_bytes() {
    let mut palette = [[0; 3]; 256];
    for (i, pixel) in palette.iter_mut().enumerate() {
        *pixel = [i as u8, (255 - i) as u8, 93];
    }
    palette[17] = palette[3];
    for width in [1, 7, 63, 64, 65, 257] {
        let indices: Vec<u8> = (0..width * 2)
            .map(|i| [3, 17, 192, 255, 0][i as usize % 5])
            .collect();
        let bytes =
            pcx::encode_indexed(width, 2, &indices, &palette, (96, 96), &|| Ok(())).unwrap();
        assert_eq!(bytes[65], 1);
        assert_eq!(bytes[bytes.len() - 769], 12);
        assert_eq!(&bytes[bytes.len() - 768..], palette.as_flattened());
        // An independent tiny RLE reader retains index identity, unlike an RGBA
        // roundtrip which cannot distinguish duplicate palette entries.
        let stride = (width + (width & 1)) as usize;
        let mut unpacked = Vec::new();
        let mut at = 128;
        while at < bytes.len() - 769 {
            let value = bytes[at];
            at += 1;
            if value >= 192 {
                let sample = bytes[at];
                at += 1;
                unpacked.extend(std::iter::repeat_n(sample, usize::from(value & 63)));
            } else {
                unpacked.push(value);
            }
        }
        assert_eq!(unpacked.len(), stride * 2);
        for y in 0..2 {
            assert_eq!(
                &unpacked[y * stride..y * stride + width as usize],
                &indices[y * width as usize..(y + 1) * width as usize]
            );
        }
        let expected: Vec<u8> = indices
            .iter()
            .flat_map(|i| {
                let p = palette[usize::from(*i)];
                [p[0], p[1], p[2], 255]
            })
            .collect();
        assert_eq!(image::decode(&bytes).unwrap().rgba, expected);
    }
}

#[test]
fn dimensions_indices_and_cancellation_fail_without_partial_output_or_panics() {
    for (w, h) in [
        (0, 1),
        (1, 0),
        (65535, 1),
        (65536, 1),
        (1, 65536),
        (4096, 4096),
    ] {
        assert!(pcx::encode_rgb(w, h, &[], (72, 72), &|| Ok(())).is_err());
        assert!(pcx::encode_indexed(w, h, &[], &[[0; 3]], (72, 72), &|| Ok(())).is_err());
    }
    assert!(pcx::encode_indexed(1, 1, &[1], &[[0; 3]], (72, 72), &|| Ok(())).is_err());
    assert!(pcx::encode_indexed(1, 1, &[0], &[], (72, 72), &|| Ok(())).is_err());
    assert!(pcx::encode_rgb(1, 1, &[1, 2, 3], (0, 72), &|| Ok(())).is_err());
    for indexed in [false, true] {
        let invoke = |check: &dyn Fn() -> paintcraft_core::types::Result<()>| {
            if indexed {
                pcx::encode_indexed(7, 3, &[0; 21], &[[17, 51, 93]], (72, 72), check)
            } else {
                pcx::encode_rgb(7, 3, &[17; 63], (72, 72), check)
            }
        };
        let count = Cell::new(0);
        invoke(&|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=count.get() {
            let calls = Cell::new(0);
            let error = invoke(&|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
        }
    }
}

#[test]
fn rgba_export_requires_alpha_consent_and_uses_an_exact_white_matte() {
    assert_eq!(
        ImageFormat::Pcx.export_losses_for_rgba(&[1, 2, 3, 127]),
        ["alpha-discarded"]
    );
    assert!(
        ImageFormat::Pcx
            .export_losses_for_rgba(&[1, 2, 3, 255])
            .is_empty()
    );
    let bytes = image::encode(
        ImageFormat::Pcx,
        3,
        1,
        &[17, 51, 93, 255, 17, 51, 93, 128, 17, 51, 93, 0],
        90,
    )
    .unwrap();
    assert_eq!(
        image::decode(&bytes).unwrap().rgba,
        [17, 51, 93, 255, 136, 153, 174, 255, 255, 255, 255, 255]
    );
    let doc = paintcraft_document::new_document(
        1,
        1,
        Some(paintcraft_core::types::Rgba([17, 51, 93, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    assert_eq!(
        paintcraft_formats::export::export_flat(&doc, ImageFormat::Pcx, 90, &[])
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let exported = paintcraft_formats::export::export_flat(
        &doc,
        ImageFormat::Pcx,
        90,
        &["alpha-discarded".into()],
    )
    .unwrap();
    assert_eq!(exported.losses, ["alpha-discarded"]);
}

#[test]
#[ignore = "requires Pillow via PAINTCRAFT_PILLOW_PYTHON and an independent FFmpeg binary"]
fn independent_pillow_and_ffmpeg_check_rgb_pixels_palette_order_and_indices() {
    let folder = std::env::temp_dir().join(format!("paintcraft-pcx-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let palette: [[u8; 3]; 256] = std::array::from_fn(|i| {
        if i == 17 {
            [3, 252, 93]
        } else {
            [i as u8, (255 - i) as u8, 93]
        }
    });
    std::fs::write(folder.join("palette.rgb"), palette.as_flattened()).unwrap();
    for width in [1, 7, 63, 64, 65, 257] {
        let indices: Vec<u8> = (0..width * 3)
            .map(|i| [3, 17, 192, 255, 0][i as usize % 5])
            .collect();
        let rgb: Vec<u8> = indices
            .iter()
            .flat_map(|i| palette[usize::from(*i)])
            .collect();
        std::fs::write(folder.join(format!("{width}.indices")), &indices).unwrap();
        std::fs::write(folder.join(format!("{width}.rgb")), &rgb).unwrap();
        std::fs::write(
            folder.join(format!("{width}-rgb.pcx")),
            pcx::encode_rgb(width, 3, &rgb, (72, 144), &|| Ok(())).unwrap(),
        )
        .unwrap();
        std::fs::write(
            folder.join(format!("{width}-indexed.pcx")),
            pcx::encode_indexed(width, 3, &indices, &palette, (72, 144), &|| Ok(())).unwrap(),
        )
        .unwrap();
    }
    let python = std::env::var_os("PAINTCRAFT_PILLOW_PYTHON").expect("PAINTCRAFT_PILLOW_PYTHON");
    let output = std::process::Command::new(python)
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/pcx-fixtures/verify_writer.py"
        ))
        .arg(&folder)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    std::fs::remove_dir_all(folder).unwrap();
}
