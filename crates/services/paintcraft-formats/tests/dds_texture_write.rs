//! Locked writer specimens are qualified independently, not by this reader.
use paintcraft_formats::dds::{
    self, Encoding, MipCount, MipFilter, SourceAlpha, TextureOptions, TextureQuality, Transfer,
};
use std::{fs, io::Write, path::PathBuf};
fn source(width: u32, height: u32, opaque: bool) -> Vec<u8> {
    (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| {
                [
                    ((x * 23 + y * 7) % 256) as u8,
                    ((x * 9 + y * 31 + 13) % 256) as u8,
                    ((x * 17 + y * 11 + 71) % 256) as u8,
                    if opaque {
                        255
                    } else {
                        [0, 51, 127, 204, 255][((x + y) % 5) as usize]
                    },
                ]
            })
        })
        .collect()
}
#[test]
fn dds_texture_vectors_match_independently_measured_frozen_specimens() {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/dds-texture-write-fixtures");
    let emit = std::env::var_os("PAINTCRAFT_DDS_WRITE_VECTORS").map(PathBuf::from);
    let quality = match std::env::var("PAINTCRAFT_DDS_WRITE_QUALITY").as_deref() {
        Ok("High") => TextureQuality::High,
        Err(_) | Ok("Normal") => TextureQuality::Normal,
        _ => panic!("Explicit Normal or High writer effort required"),
    };
    let qualities = if emit.is_some() {
        vec![quality]
    } else {
        vec![TextureQuality::Normal, TextureQuality::High]
    };
    for quality in qualities {
        let fixture = fixtures.join(if quality == TextureQuality::Normal {
            "normal"
        } else {
            "high"
        });
        let mut timings = emit.as_ref().map(|path| {
            fs::create_dir_all(path).unwrap();
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path.join("timings.tsv"))
                .unwrap()
        });
        for (width, height) in [(7, 5), (1, 17), (17, 1)] {
            for encoding in [Encoding::Rgba8, Encoding::Bc1, Encoding::Bc2, Encoding::Bc3] {
                let opaque = encoding == Encoding::Bc1;
                let rgba = source(width, height, opaque);
                for transfer in [Transfer::Linear, Transfer::Srgb] {
                    for filter in [MipFilter::Nearest, MipFilter::BoxStored] {
                        let mut name =
                            format!("{width}x{height}-{encoding:?}-{transfer:?}-{filter:?}");
                        if quality == TextureQuality::High {
                            name.push_str("-High");
                        }
                        let options = TextureOptions {
                            encoding,
                            transfer,
                            filter,
                            quality,
                            mips: MipCount::Full,
                            alpha: if opaque {
                                SourceAlpha::Opaque
                            } else {
                                SourceAlpha::Straight
                            },
                        };
                        let started = std::time::Instant::now();
                        let output = dds::encode_texture_checked(
                            width,
                            height,
                            &rgba,
                            options,
                            |plan| {
                                assert!(plan.reservation_bytes().unwrap() < 64 << 20);
                                Ok(())
                            },
                            &|| Ok(()),
                        )
                        .unwrap();
                        if let Some(timings) = &mut timings {
                            writeln!(timings, "{name}\t{}", started.elapsed().as_nanos()).unwrap();
                        }
                        if let Some(path) = &emit {
                            // Output generation is deliberately opt-in and cannot replace evidence.
                            fs::OpenOptions::new()
                                .create_new(true)
                                .write(true)
                                .open(path.join(format!("{name}.dds")))
                                .unwrap()
                                .write_all(output.bytes())
                                .unwrap();
                            fs::OpenOptions::new()
                                .create_new(true)
                                .write(true)
                                .open(path.join(format!("{name}.rgba")))
                                .unwrap()
                                .write_all(&rgba)
                                .unwrap();
                        } else {
                            assert_eq!(
                                output.bytes(),
                                fs::read(fixture.join(format!("{name}.dds"))).unwrap(),
                                "qualified output drift for {name}"
                            );
                        }
                    }
                }
            }
        }
    }
}
