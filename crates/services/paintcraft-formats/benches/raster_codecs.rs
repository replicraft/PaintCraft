//! Codec workload measurements, not equivalence or optimized-backend comparisons.
//! Correctness and manifest generation run before any timed sample.
use paintcraft_formats::image::{self, ImageFormat};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt::Write as _, sync::OnceLock};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

const CASES: &[&str] = &[
    "64-sparse-opaque",
    "64-dense-opaque",
    "64-dense-alpha",
    "64-palette",
    "1000x486-sparse-opaque",
    "1000x486-dense-opaque",
    "1000x486-dense-alpha",
    "1000x486-palette",
];
const FORMATS: [ImageFormat; 6] = [
    ImageFormat::Png,
    ImageFormat::Bmp,
    ImageFormat::Jpeg,
    ImageFormat::Gif,
    ImageFormat::Tiff,
    ImageFormat::Pcx,
];
struct Fixture {
    name: &'static str,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    encoded: [Vec<u8>; 6],
}
static FIXTURES: OnceLock<Vec<Fixture>> = OnceLock::new();

fn fixtures() -> &'static [Fixture] {
    FIXTURES.get_or_init(|| {
        CASES
            .iter()
            .map(|&name| {
                let (width, height): (u32, u32) = if name.starts_with("64-") {
                    (64, 64)
                } else {
                    (1000, 486)
                };
                let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
                for y in 0..height {
                    for x in 0..width {
                        let hash = x.wrapping_mul(0x9e3779b9) ^ y.wrapping_mul(0x85ebca6b);
                        let rgb = [hash as u8, (hash >> 8) as u8, (hash >> 16) as u8];
                        let sample = if name.ends_with("sparse-opaque") {
                            if x >= width / 3
                                && x < width / 3 + width / 16
                                && y >= height / 3
                                && y < height / 3 + height / 16
                            {
                                [20, 80, 180, 255]
                            } else {
                                [255, 255, 255, 255]
                            }
                        } else if name.ends_with("palette") {
                            let index = ((x / 7 + y / 5) % 8) as u8;
                            [
                                if index & 1 != 0 { 255 } else { 0 },
                                if index & 2 != 0 { 255 } else { 0 },
                                if index & 4 != 0 { 255 } else { 0 },
                                255,
                            ]
                        } else {
                            [
                                rgb[0],
                                rgb[1],
                                rgb[2],
                                if name.ends_with("alpha") {
                                    match (x + y) % 5 {
                                        0 => 0,
                                        1 => 1,
                                        2 => 128,
                                        _ => 255,
                                    }
                                } else {
                                    255
                                },
                            ]
                        };
                        rgba.extend(sample);
                    }
                }
                let encoded =
                    FORMATS.map(|format| image::encode(format, width, height, &rgba, 90).unwrap());
                Fixture {
                    name,
                    width,
                    height,
                    rgba,
                    encoded,
                }
            })
            .collect()
    })
}
fn fixture(case: &str) -> &'static Fixture {
    fixtures().iter().find(|f| f.name == case).unwrap()
}

fn preflight() {
    for fixture in fixtures() {
        for (index, format) in FORMATS.into_iter().enumerate() {
            let decoded = image::decode(&fixture.encoded[index]).unwrap();
            assert_eq!(decoded.format, format);
            assert_eq!(
                (decoded.width, decoded.height),
                (fixture.width, fixture.height)
            );
            match format {
                ImageFormat::Png
                | ImageFormat::Bmp
                | ImageFormat::Tiff
                | ImageFormat::Tga
                | ImageFormat::WebP => {
                    assert_eq!(decoded.rgba, fixture.rgba)
                }
                ImageFormat::Gif => {
                    if fixture.name.ends_with("palette") || fixture.name.ends_with("sparse-opaque")
                    {
                        assert_eq!(
                            decoded.rgba, fixture.rgba,
                            "exact <=256 opaque palette case"
                        );
                    } else {
                        assert!(
                            decoded
                                .rgba
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .all(|p| matches!(p[3], 0 | 255))
                        );
                        assert!(
                            decoded
                                .rgba
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .collect::<BTreeSet<_>>()
                                .len()
                                <= 256
                        );
                    }
                    for (source, output) in fixture
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(decoded.rgba.as_chunks::<4>().0.iter())
                    {
                        assert_eq!(
                            output[3],
                            if source[3] == 0 { 0 } else { 255 },
                            "GIF binary alpha policy"
                        );
                    }
                    assert!(
                        decoded
                            .losses
                            .iter()
                            .any(|loss| loss == "palette-indices-expanded")
                    );
                }
                ImageFormat::Jpeg => {
                    assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255))
                }
                ImageFormat::Pcx => {
                    for (source, actual) in fixture
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(decoded.rgba.as_chunks::<4>().0)
                    {
                        let alpha = u32::from(source[3]);
                        let mut expected = [255; 4];
                        for channel in 0..3 {
                            expected[channel] =
                                ((u32::from(source[channel]) * alpha + 255 * (255 - alpha) + 127)
                                    / 255) as u8;
                        }
                        assert_eq!(*actual, expected);
                    }
                }
                ImageFormat::Msp
                | ImageFormat::Qoi
                | ImageFormat::Pnm
                | ImageFormat::Farbfeld
                | ImageFormat::Jxl
                | ImageFormat::Dds
                | ImageFormat::Avif => {
                    panic!("additional formats need dedicated independent benchmark fixtures")
                }
            }
        }
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn manifest() {
    let Ok(directory) = std::env::var("PAINTCRAFT_BENCH_FIXTURES") else {
        return;
    };
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut csv = String::from(
        "case,format,width,height,rgba_bytes,encoded_bytes,source_sha256,encoded_sha256,export_losses\n",
    );
    for fixture in fixtures() {
        for (index, format) in FORMATS.into_iter().enumerate() {
            let encoded = &fixture.encoded[index];
            let suffix = format.name().to_lowercase();
            std::fs::write(
                directory.join(format!("{}.{}", fixture.name, suffix)),
                encoded,
            )
            .unwrap();
            writeln!(
                csv,
                "{},{},{},{},{},{},{},{},{}",
                fixture.name,
                format.name(),
                fixture.width,
                fixture.height,
                fixture.rgba.len(),
                encoded.len(),
                digest(&fixture.rgba),
                digest(encoded),
                format.export_losses_for_rgba(&fixture.rgba).join(";")
            )
            .unwrap();
        }
    }
    std::fs::write(directory.join("fixtures.csv"), csv).unwrap();
}
fn main() {
    preflight();
    manifest();
    divan::main();
}
fn encode(bencher: divan::Bencher, case: &str, index: usize) {
    let fixture = fixture(case);
    bencher
        .counter(divan::counter::BytesCount::new(fixture.rgba.len()))
        .bench_local(|| {
            divan::black_box_drop(
                image::encode(
                    FORMATS[index],
                    fixture.width,
                    fixture.height,
                    divan::black_box(&fixture.rgba),
                    90,
                )
                .unwrap(),
            )
        });
}
fn decode(bencher: divan::Bencher, case: &str, index: usize) {
    let fixture = fixture(case);
    // Throughput denominator is decoded RGBA8 bytes for both directions.
    bencher
        .counter(divan::counter::BytesCount::new(fixture.rgba.len()))
        .bench_local(|| {
            divan::black_box_drop(image::decode(divan::black_box(&fixture.encoded[index])).unwrap())
        });
}
macro_rules! benches {
    ($encode:ident,$decode:ident,$index:expr) => {
        #[divan::bench(args=CASES)]
        fn $encode(bencher: divan::Bencher, case: &str) {
            encode(bencher, case, $index)
        }
        #[divan::bench(args=CASES)]
        fn $decode(bencher: divan::Bencher, case: &str) {
            decode(bencher, case, $index)
        }
    };
}
benches!(encode_png, decode_png, 0);
benches!(encode_bmp, decode_bmp, 1);
benches!(encode_jpeg, decode_jpeg, 2);
benches!(encode_gif, decode_gif, 3);
benches!(encode_tiff, decode_tiff, 4);

benches!(encode_pcx, decode_pcx, 5);
