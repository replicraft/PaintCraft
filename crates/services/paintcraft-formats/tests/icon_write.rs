use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::icons::{self, Encoding, Kind, WriteEntry, WriteLimits};
use std::cell::Cell;

fn pixels(width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                (x * 11) as u8,
                (y * 13) as u8,
                ((x + y) * 7) as u8,
                ((x * 19 + y * 23) % 256) as u8,
            ]);
        }
    }
    // Keep one nonzero alpha even in a 1x1 bitmap entry.
    *rgba.last_mut().unwrap() = 127;
    rgba
}

#[test]
fn mixed_asset_sets_preserve_all_entries_pixels_hotspots_and_explicit_encoding() {
    for kind in [Kind::Icon, Kind::Cursor] {
        let small = pixels(17, 9);
        let large = pixels(256, 256);
        let opaque = [11, 22, 33, 255].repeat(16 * 16);
        let transparent = [77, 88, 99, 0].repeat(32 * 32);
        let entries = [
            WriteEntry {
                width: 17,
                height: 9,
                rgba: &small,
                encoding: Encoding::Bitmap,
                hotspot: (kind == Kind::Cursor).then_some((16, 8)),
            },
            WriteEntry {
                width: 256,
                height: 256,
                rgba: &large,
                encoding: Encoding::Png,
                hotspot: (kind == Kind::Cursor).then_some((255, 255)),
            },
            WriteEntry {
                width: 16,
                height: 16,
                rgba: &opaque,
                encoding: Encoding::Png,
                hotspot: (kind == Kind::Cursor).then_some((0, 0)),
            },
            WriteEntry {
                width: 32,
                height: 32,
                rgba: &transparent,
                encoding: Encoding::Png,
                hotspot: (kind == Kind::Cursor).then_some((7, 11)),
            },
        ];
        let bytes =
            icons::encode_entries(kind, &entries, WriteLimits::default(), &|| Ok(())).unwrap();
        let directory = icons::inspect(&bytes).unwrap();
        assert_eq!(directory.kind, kind);
        assert_eq!(directory.entries.len(), entries.len());
        assert_eq!(
            icons::encode_entries(kind, &entries, WriteLimits::default(), &|| Ok(())).unwrap(),
            bytes
        );
        for (index, input) in entries.iter().enumerate() {
            let decoded = icons::decode_entry(&bytes, index as u16, 65536).unwrap();
            assert_eq!(
                (decoded.entry.width, decoded.entry.height),
                (input.width, input.height)
            );
            assert_eq!(decoded.entry.hotspot, input.hotspot);
            assert_eq!(decoded.entry.encoding, input.encoding);
            assert_eq!(decoded.entry.bits_per_pixel, 32);
            assert_eq!(decoded.rgba, input.rgba);
            let offset = decoded.entry.offset as usize;
            if input.encoding == Encoding::Png {
                assert_eq!(bytes[offset + 25], 6);
            }
        }
    }
}

#[test]
fn rejects_invalid_entries_and_aggregate_budgets_before_encoding() {
    let rgba = [1, 2, 3, 127];
    let good = WriteEntry {
        width: 1,
        height: 1,
        rgba: &rgba,
        encoding: Encoding::Bitmap,
        hotspot: None,
    };
    for invalid in [
        WriteEntry { width: 0, ..good },
        WriteEntry { width: 257, ..good },
        WriteEntry {
            rgba: &[0; 3],
            ..good
        },
        WriteEntry {
            hotspot: Some((0, 0)),
            ..good
        },
        WriteEntry {
            rgba: &[0; 4],
            ..good
        },
    ] {
        let calls = Cell::new(0);
        assert!(
            icons::encode_entries(
                Kind::Icon,
                &[good, invalid],
                WriteLimits::default(),
                &|| {
                    calls.set(calls.get() + 1);
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
    for entries in [vec![], vec![good; icons::ENTRIES_MAX + 1]] {
        assert_eq!(
            icons::encode_entries(Kind::Icon, &entries, WriteLimits::default(), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    for limits in [
        WriteLimits {
            pixels: 1,
            ..Default::default()
        },
        WriteLimits {
            entries: 1,
            ..Default::default()
        },
        WriteLimits {
            encoded_bytes: 21,
            ..Default::default()
        },
    ] {
        assert_eq!(
            icons::encode_entries(Kind::Icon, &[good, good], limits, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    assert!(
        icons::encode_entries(Kind::Cursor, &[good], WriteLimits::default(), &|| Ok(())).is_err()
    );
    assert!(
        icons::encode_entries(
            Kind::Cursor,
            &[WriteEntry {
                hotspot: Some((1, 0)),
                ..good
            }],
            WriteLimits::default(),
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        icons::encode_entries(
            Kind::Icon,
            &[good],
            WriteLimits {
                encoded_bytes: 23,
                ..Default::default()
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn cancellation_at_every_boundary_returns_the_original_error_without_output() {
    let rgba = pixels(3, 2);
    let entries = [Encoding::Bitmap, Encoding::Png].map(|encoding| WriteEntry {
        width: 3,
        height: 2,
        rgba: &rgba,
        encoding,
        hotspot: None,
    });
    let calls = Cell::new(0);
    icons::encode_entries(Kind::Icon, &entries, WriteLimits::default(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 0..calls.get() {
        let visited = Cell::new(0);
        let error = icons::encode_entries(Kind::Icon, &entries, WriteLimits::default(), &|| {
            let index = visited.get();
            visited.set(index + 1);
            if index == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel icon test",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled, "boundary {stop}");
        assert_eq!(error.message, "cancel icon test");
    }
}

#[test]
#[ignore = "requires PAINTCRAFT_PILLOW_PYTHON with Pillow"]
fn independent_pillow_reads_fresh_icon_entries() {
    let python = std::env::var_os("PAINTCRAFT_PILLOW_PYTHON")
        .expect("select the independent Python interpreter");
    let folder =
        std::env::temp_dir().join(format!("paintcraft-icon-writer-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let sizes = [1, 17, 32, 48, 256];
    let sources: Vec<_> = sizes.iter().map(|&size| pixels(size, size)).collect();
    for kind in [Kind::Icon, Kind::Cursor] {
        for encoding in [Encoding::Bitmap, Encoding::Png] {
            let entries: Vec<_> = sizes
                .iter()
                .zip(&sources)
                .map(|(&size, rgba)| WriteEntry {
                    width: size,
                    height: size,
                    rgba,
                    encoding,
                    hotspot: (kind == Kind::Cursor).then_some(((size - 1) as u16, 0)),
                })
                .collect();
            let bytes =
                icons::encode_entries(kind, &entries, WriteLimits::default(), &|| Ok(())).unwrap();
            let suffix = if kind == Kind::Icon { "ico" } else { "cur" };
            let profile = if encoding == Encoding::Png {
                "png"
            } else {
                "bitmap"
            };
            std::fs::write(folder.join(format!("{profile}.{suffix}")), bytes).unwrap();
        }
    }
    for (&size, rgba) in sizes.iter().zip(&sources) {
        std::fs::write(folder.join(format!("{size}.rgba")), rgba).unwrap();
    }
    let special = [
        (16, [11, 22, 33, 255].repeat(16 * 16)),
        (32, [77, 88, 99, 0].repeat(32 * 32)),
    ];
    for (size, rgba) in &special {
        std::fs::write(folder.join(format!("special-{size}.rgba")), rgba).unwrap();
    }
    for kind in [Kind::Icon, Kind::Cursor] {
        let entries: Vec<_> = special
            .iter()
            .map(|(size, rgba)| WriteEntry {
                width: *size,
                height: *size,
                rgba,
                encoding: Encoding::Png,
                hotspot: (kind == Kind::Cursor).then_some(((*size - 1) as u16, 0)),
            })
            .collect();
        let bytes =
            icons::encode_entries(kind, &entries, WriteLimits::default(), &|| Ok(())).unwrap();
        std::fs::write(
            folder.join(if kind == Kind::Icon {
                "special.ico"
            } else {
                "special.cur"
            }),
            bytes,
        )
        .unwrap();
    }
    let output = std::process::Command::new(python)
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/icon-fixtures/verify_writer.py"
        ))
        .arg(&folder)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    std::fs::remove_dir_all(folder).unwrap();
}
