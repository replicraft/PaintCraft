//! ASE baseline workloads. Correctness and teardown are part of every real path.
use paintcraft_formats::ase::{self, Color, Entry, Kind, Palette, Swatch};
use sha2::{Digest, Sha256};
use std::{fmt::Write as _, sync::OnceLock, time::Instant};
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();
const CASES: &[&str] = &["empty-0", "mixed-6", "grouped-256", "grouped-4096"];
struct Fixture {
    name: &'static str,
    palette: Palette,
    bytes: Vec<u8>,
}
static FIXTURES: OnceLock<Vec<Fixture>> = OnceLock::new();
fn fixtures() -> &'static [Fixture] {
    FIXTURES.get_or_init(|| {
        CASES
            .iter()
            .map(|&name| {
                let bytes = match name {
                    "empty-0" => include_bytes!("../tests/ase-fixtures/empty.ase").to_vec(),
                    "mixed-6" => include_bytes!("../tests/ase-fixtures/mixed.ase").to_vec(),
                    _ => {
                        let count = if name == "grouped-256" { 256 } else { 4096 };
                        let groups = (0..count / 32)
                            .map(|group| Entry::Group {
                                name: format!("Group {group}"),
                                swatches: (group * 32..group * 32 + 32)
                                    .map(|i| {
                                        let q = (i % 17) as f32 / 16.0;
                                        let color = match i % 4 {
                                            0 => Color::Rgb([q, (i % 9) as f32 / 8.0, 1.0]),
                                            1 => Color::Cmyk([q, 0.5, 0.25, 1.0]),
                                            2 => Color::Gray(q),
                                            _ => Color::Lab([
                                                q,
                                                (i % 128) as f32 - 64.0,
                                                (i % 64) as f32 - 32.0,
                                            ]),
                                        };
                                        Swatch {
                                            name: if i % 8 == 0 {
                                                format!("色 {i} 🚀")
                                            } else {
                                                format!("Color {i}")
                                            },
                                            color,
                                            kind: match i % 3 {
                                                0 => Kind::Global,
                                                1 => Kind::Spot,
                                                _ => Kind::Normal,
                                            },
                                        }
                                    })
                                    .collect(),
                            })
                            .collect();
                        ase::encode(&Palette::new(groups, &|| Ok(())).unwrap(), &|| Ok(())).unwrap()
                    }
                };
                let palette = ase::decode(&bytes, &|| Ok(())).unwrap();
                assert_eq!(ase::encode(&palette, &|| Ok(())).unwrap(), bytes);
                assert_eq!(
                    ase::decode(&ase::encode(&palette, &|| Ok(())).unwrap(), &|| Ok(())).unwrap(),
                    palette
                );
                Fixture {
                    name,
                    palette,
                    bytes,
                }
            })
            .collect()
    })
}
fn fixture(name: &str) -> &'static Fixture {
    fixtures().iter().find(|f| f.name == name).unwrap()
}
fn publish(path: &std::path::Path, bytes: &[u8]) {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}
fn manifest() {
    let Ok(path) = std::env::var("PAINTCRAFT_PALETTE_FIXTURES") else {
        return;
    };
    let path = std::path::Path::new(&path);
    std::fs::create_dir_all(path).unwrap();
    let mut rows = String::from("case,swatches,encoded_bytes,sha256\n");
    for f in fixtures() {
        let sha = paintcraft_document::hex(&Sha256::digest(&f.bytes));
        writeln!(
            rows,
            "{},{},{},{}",
            f.name,
            f.palette.swatch_count(),
            f.bytes.len(),
            sha
        )
        .unwrap();
        publish(&path.join(format!("{}.ase", f.name)), &f.bytes);
    }
    publish(&path.join("fixtures.csv"), rows.as_bytes());
}
/// Supplemental raw wall-clock batches. Divan separately supplies calibrated
/// statistics/allocation counters. Setup is excluded; every result is dropped.
fn raw_samples() {
    let Ok(path) = std::env::var("PAINTCRAFT_PALETTE_SAMPLES") else {
        return;
    };
    let mut rows = String::from("case,operation,sample,iterations,elapsed_ns\n");
    let mut ordered: Vec<_> = fixtures().iter().collect();
    if std::env::var_os("PAINTCRAFT_PALETTE_REVERSE").is_some() {
        ordered.reverse();
    }
    for f in ordered {
        for operation in ["decode", "encode"] {
            for sample in 0..33 {
                let started = Instant::now();
                for _ in 0..16 {
                    if operation == "decode" {
                        divan::black_box_drop(
                            ase::decode(divan::black_box(&f.bytes), &|| Ok(())).unwrap(),
                        );
                    } else {
                        divan::black_box_drop(
                            ase::encode(divan::black_box(&f.palette), &|| Ok(())).unwrap(),
                        );
                    }
                }
                let ns = started.elapsed().as_nanos();
                if sample >= 3 {
                    writeln!(rows, "{},{},{},16,{}", f.name, operation, sample - 3, ns).unwrap();
                }
            }
        }
    }
    publish(std::path::Path::new(&path), rows.as_bytes());
}
fn main() {
    fixtures();
    manifest();
    raw_samples();
    divan::main();
}
#[divan::bench(args=CASES)]
fn decode(bencher: divan::Bencher, case: &str) {
    let f = fixture(case);
    bencher
        .counter(divan::counter::BytesCount::new(f.bytes.len()))
        .bench_local(|| {
            divan::black_box_drop(ase::decode(divan::black_box(&f.bytes), &|| Ok(())).unwrap());
        });
}
#[divan::bench(args=CASES)]
fn encode(bencher: divan::Bencher, case: &str) {
    let f = fixture(case);
    bencher
        .counter(divan::counter::BytesCount::new(f.bytes.len()))
        .bench_local(|| {
            divan::black_box_drop(ase::encode(divan::black_box(&f.palette), &|| Ok(())).unwrap());
        });
}
