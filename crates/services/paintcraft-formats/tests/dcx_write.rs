//! Fresh DCX pages through the selected PCX encoders, independent of frontend gates.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{dcx, image};
use std::{cell::Cell, io::Write};
fn palette(page: u8) -> [[u8; 3]; 256] {
    let mut result = std::array::from_fn(|i| {
        [
            (i as u8).wrapping_add(page * 31),
            (i as u8).wrapping_mul(43).wrapping_add(page * 11),
            (255 - i as u8).wrapping_add(page * 7),
        ]
    });
    result[17] = result[3];
    result
}
fn rgb(page: u8, width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|i| {
            if i % 5 == 0 {
                [193, 255, 192]
            } else {
                [17, 51, (i as u8).wrapping_add(page * 43)]
            }
        })
        .collect()
}
fn indices(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .map(|i| [3, 17, 192, 255, 0, 7][i as usize % 6])
        .collect()
}
#[test]
fn mixed_pages_retain_order_geometry_dpi_index_identity_and_distinct_palettes() {
    let rgb = rgb(0, 3, 2);
    let indices = indices(7, 2);
    let palette = palette(1);
    let pages = [
        dcx::WritePage {
            width: 3,
            height: 2,
            dpi: (72, 144),
            pixels: dcx::PagePixels::Rgb(&rgb),
        },
        dcx::WritePage {
            width: 7,
            height: 2,
            dpi: (96, 97),
            pixels: dcx::PagePixels::Indexed {
                indices: &indices,
                palette: &palette,
            },
        },
    ];
    let plan = dcx::preflight_write(&pages, &Default::default(), &|| Ok(())).unwrap();
    let bytes = dcx::encode_pages(&pages, &Default::default(), &|| Ok(())).unwrap();
    assert!(bytes.len() <= plan.output_capacity);
    assert_eq!(
        bytes,
        dcx::encode_pages(&pages, &Default::default(), &|| Ok(())).unwrap()
    );
    let info = dcx::inspect(&bytes).unwrap();
    assert_eq!(info.len(), 2);
    assert_eq!(info[0].offset, 4100);
    assert_eq!((info[0].width, info[0].height), (3, 2));
    assert_eq!((info[1].width, info[1].height), (7, 2));
    for index in 2..1024 {
        assert_eq!(&bytes[4 + index * 4..8 + index * 4], &[0; 4]);
    }
    let decoded = dcx::decode_page(&bytes, 0, 100).unwrap();
    let expected: Vec<u8> = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    assert_eq!(decoded.rgba, expected);
    let page = &bytes[info[1].offset as usize..];
    assert_eq!(&page[page.len() - 768..], palette.as_flattened());
    assert_eq!(&page[12..16], &[96, 0, 97, 0]);
    let expected: Vec<_> = indices
        .iter()
        .flat_map(|index| {
            let p = palette[usize::from(*index)];
            [p[0], p[1], p[2], 255]
        })
        .collect();
    assert_eq!(image::decode(page).unwrap().rgba, expected);
    let original = bytes.clone();
    for offset in [0u32, 4100, u32::MAX] {
        let mut bad = bytes.clone();
        bad[8..12].copy_from_slice(&offset.to_le_bytes());
        assert!(dcx::inspect(&bad).is_err());
    }
    let mut bad = bytes.clone();
    bad[12..16].copy_from_slice(&4100u32.to_le_bytes());
    assert!(dcx::inspect(&bad).is_err());
    bad = bytes;
    bad[info[1].offset as usize] = 0;
    assert!(dcx::decode_page(&bad, 0, 100).is_err());
    assert_ne!(original, bad);
}
#[test]
fn all_pages_are_admitted_before_output_and_directory_limit_is_exact() {
    let rgb = [1, 2, 3];
    let page = dcx::WritePage {
        width: 1,
        height: 1,
        dpi: (1, 1),
        pixels: dcx::PagePixels::Rgb(&rgb),
    };
    let pages = vec![page; 1023];
    let bytes = dcx::encode_pages(&pages, &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(dcx::inspect(&bytes).unwrap().len(), 1023);
    assert_eq!(&bytes[4096..4100], &[0; 4]);
    let pages = vec![page; 1024];
    assert!(dcx::encode_pages(&pages, &Default::default(), &|| Ok(())).is_err());
    assert!(dcx::encode_pages(&[], &Default::default(), &|| Ok(())).is_err());
    for case in 0..8 {
        let mut invalid = page;
        let mut limits = dcx::WriteLimits::default();
        match case {
            0 => invalid.width = 65535,
            1 => invalid.dpi = (0, 1),
            2 => invalid.pixels = dcx::PagePixels::Rgb(&rgb[..2]),
            3 => {
                invalid.pixels = dcx::PagePixels::Indexed {
                    indices: &[255],
                    palette: &[[0, 0, 0]],
                }
            }
            4 => limits.pixels = 1,
            5 => limits.encoded_bytes = 4100,
            6 => limits.transient_bytes = 1,
            _ => limits.pages = 1,
        };
        let pages = [page, invalid];
        let mut sink = Vec::new();
        assert!(
            dcx::write_pages_to(&pages, &limits, &mut sink, &|| Ok(())).is_err(),
            "case{case}"
        );
        assert!(sink.is_empty());
    }
}
#[test]
fn every_encoding_cancellation_checkpoint_and_failed_private_sink_preserve_sources() {
    let rgb = rgb(0, 257, 2);
    let before = rgb.clone();
    let pages = [dcx::WritePage {
        width: 257,
        height: 2,
        dpi: (72, 72),
        pixels: dcx::PagePixels::Rgb(&rgb),
    }; 3];
    let count = Cell::new(0);
    dcx::encode_pages(&pages, &Default::default(), &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let mut sink = Vec::new();
        let result = dcx::write_pages_to(&pages, &Default::default(), &mut sink, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert!(sink.is_empty());
        assert_eq!(rgb, before);
    }
    struct Failed;
    impl Write for Failed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        dcx::write_pages_to(&pages, &Default::default(), &mut Failed, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::StorageFailed
    );
}
#[test]
fn independently_verified_vectors_stay_identical_and_optional_export_is_create_new() {
    let folder = std::env::var_os("PAINTCRAFT_DCX_WRITER_OUTPUT").map(std::path::PathBuf::from);
    if let Some(folder) = &folder {
        assert!(folder.is_dir());
        assert_eq!(std::fs::read_dir(folder).unwrap().count(), 0);
    }
    let shapes = [(3, 2), (65, 2), (1, 5)];
    let rgbs: [Vec<u8>; 3] = std::array::from_fn(|i| rgb(i as u8, shapes[i].0, shapes[i].1));
    let indices = shapes.map(|(w, h)| indices(w, h));
    let palettes = [palette(0), palette(1), palette(2)];
    for kind in ["rgb", "indexed", "mixed"] {
        let pages: Vec<_> = shapes
            .iter()
            .enumerate()
            .map(|(i, (width, height))| dcx::WritePage {
                width: *width,
                height: *height,
                dpi: (72 + i as u16, 144 + i as u16),
                pixels: if kind == "rgb" || kind == "mixed" && i == 1 {
                    dcx::PagePixels::Rgb(&rgbs[i])
                } else {
                    dcx::PagePixels::Indexed {
                        indices: &indices[i],
                        palette: &palettes[i],
                    }
                },
            })
            .collect();
        let bytes = dcx::encode_pages(&pages, &Default::default(), &|| Ok(())).unwrap();
        let frozen = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/dcx-fixtures/writer")
            .join(format!("{kind}.dcx"));
        assert_eq!(
            bytes,
            std::fs::read(frozen).unwrap(),
            "qualified {kind} drift"
        );
        let Some(folder) = &folder else { continue };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join(format!("{kind}.dcx")))
            .unwrap();
        file.write_all(&bytes).unwrap();
    }
}
