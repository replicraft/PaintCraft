//! Benchmark caller only: actual standalone row kernel, no editor/UI claim.
use paintcraft_core::types::{
    Error, ErrorCode, Result, Rgba,
    coordinates::SourcePixel,
    filters::{AddNoiseRgbV1, NoiseDistribution},
};
use paintcraft_effects::noise;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    hint::black_box,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy)]
pub struct Case {
    pub id: &'static str,
    pub width: usize,
    pub height: usize,
    pub mono: bool,
    pub distribution: NoiseDistribution,
    pub optional: bool,
}
macro_rules! cases {
    ($label:literal,$w:literal,$h:literal,$optional:literal) => {
        [
            Case {
                id: concat!($label, "-uniform-mono"),
                width: $w,
                height: $h,
                mono: true,
                distribution: NoiseDistribution::Uniform,
                optional: $optional,
            },
            Case {
                id: concat!($label, "-uniform-color"),
                width: $w,
                height: $h,
                mono: false,
                distribution: NoiseDistribution::Uniform,
                optional: $optional,
            },
            Case {
                id: concat!($label, "-gaussian-mono"),
                width: $w,
                height: $h,
                mono: true,
                distribution: NoiseDistribution::Gaussian,
                optional: $optional,
            },
            Case {
                id: concat!($label, "-gaussian-color"),
                width: $w,
                height: $h,
                mono: false,
                distribution: NoiseDistribution::Gaussian,
                optional: $optional,
            },
        ]
    };
}
pub const CASE_GROUPS: [[Case; 4]; 3] = [
    cases!("default", 1000, 486, false),
    cases!("1080p", 1920, 1080, false),
    cases!("exact-2MP", 2000, 1000, true),
];
pub fn cases() -> impl Iterator<Item = Case> {
    CASE_GROUPS.into_iter().flatten()
}
pub fn find(id: &str) -> Case {
    cases()
        .find(|c| c.id == id)
        .expect("exact reviewed case id")
}
fn fail(message: &'static str) -> Error {
    Error::new(ErrorCode::InvariantViolation, message)
}
fn buffer(n: usize) -> Result<Vec<Rgba>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Cannot allocate benchmark caller plane",
        )
    })?;
    v.resize(n, Rgba([0; 4]));
    Ok(v)
}
pub fn source(x: usize, y: usize) -> Rgba {
    Rgba([
        ((17 * x + 31 * y + 23) & 255) as u8,
        ((29 * x + 11 * y + 71) & 255) as u8,
        ((7 * x + 43 * y + 193) & 255) as u8,
        255,
    ])
}
pub struct Frame {
    pub case: Case,
    input: Vec<Rgba>,
    output: Vec<Rgba>,
    cancel: AtomicBool,
    source_hash: String,
}
#[derive(Clone, Copy)]
pub struct Used {
    pub work: u64,
    pub counted_callbacks: u64,
}
impl Frame {
    pub fn new(case: Case) -> Result<Self> {
        let mut input = buffer(case.width * case.height.min(256))?;
        for (y, row) in input.chunks_mut(case.width).enumerate() {
            for (x, p) in row.iter_mut().enumerate() {
                *p = source(x, y);
            }
        }
        let source_hash = digest(&input);
        Ok(Self {
            case,
            input,
            output: buffer(case.width * case.height)?,
            cancel: AtomicBool::new(false),
            source_hash,
        })
    }
    pub fn policy(&self, amount: i16) -> AddNoiseRgbV1 {
        AddNoiseRgbV1::new(amount, 1, self.case.mono, self.case.distribution).unwrap()
    }
    pub fn run(&mut self, amount: i16, counted: bool) -> Result<Used> {
        let policy = black_box(self.policy(amount));
        let full = noise::quote(policy, self.case.width * self.case.height)?;
        let row_quote = noise::quote(policy, self.case.width)?;
        let calls = Cell::new(0u64);
        let cancel = black_box(&self.cancel);
        let check = || {
            if counted {
                calls.set(calls.get() + 1);
            }
            if cancel.load(Ordering::Relaxed) {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "Benchmark caller cancellation",
                ))
            } else {
                Ok(())
            }
        };
        let mut total = 0u64;
        for (y, out) in self.output.chunks_mut(self.case.width).enumerate() {
            let start = (y % self.case.height.min(256)) * self.case.width;
            let input = &self.input[start..start + self.case.width];
            let next = total
                .checked_add(row_quote.work)
                .ok_or_else(|| fail("Benchmark caller work overflow"))?;
            if next > full.work {
                return Err(fail("Benchmark caller cumulative grant exhausted"));
            }
            let used = noise::evaluate_row(
                policy,
                black_box(SourcePixel::new(0, y as u32)),
                black_box(input),
                black_box(out),
                row_quote.into(),
                &check,
            )?;
            if used.units != row_quote.work {
                return Err(fail("Benchmark actual row work differs"));
            }
            total = next;
        }
        if total != full.work {
            return Err(fail("Benchmark whole-plane quote differs"));
        }
        black_box(&self.output);
        Ok(Used {
            work: total,
            counted_callbacks: calls.get(),
        })
    }
    pub fn verify(&self, amount: i16) -> Result<()> {
        if digest(&self.input) != self.source_hash {
            return Err(fail("Benchmark source changed"));
        }
        let oracle: Value = serde_json::from_str(include_str!(
            "../../tests/noise-latency-fixtures/oracle.json"
        ))
        .unwrap();
        let mut count = 0;
        for row in oracle["rows"].as_array().unwrap().iter().filter(|r| {
            r["case"] == self.case.id && r["amount_hundredths"].as_i64() == Some(i64::from(amount))
        }) {
            let x = row["xy"][0].as_u64().unwrap() as usize;
            let y = row["xy"][1].as_u64().unwrap() as usize;
            let rgba = Rgba(std::array::from_fn(|i| {
                row["rgba"][i].as_u64().unwrap() as u8
            }));
            let expected = Rgba(std::array::from_fn(|i| {
                row["expected"][i].as_u64().unwrap() as u8
            }));
            let actual = noise::evaluate_pixel(
                self.policy(amount),
                SourcePixel::new(x as u32, y as u32),
                rgba,
            )?;
            if actual != expected {
                return Err(fail("Independent procedural scalar probe differs"));
            }
            if rgba.a() == 255 && self.output[y * self.case.width + x] != expected {
                return Err(fail("Independent procedural row probe differs"));
            }
            count += 1;
        }
        if count != 18 {
            return Err(fail("Benchmark probe count differs"));
        }
        if self.output.iter().any(|p| p.a() != 255) {
            return Err(fail("Benchmark alpha changed"));
        }
        Ok(())
    }
    pub fn digest(&self) -> String {
        digest(&self.output)
    }
    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }
    pub fn quoted_work(&self, amount: i16) -> u64 {
        noise::quote(self.policy(amount), self.case.width * self.case.height)
            .unwrap()
            .work
    }
    pub fn expected_callbacks(&self) -> u64 {
        (self.case.height * (2 + self.case.width.div_ceil(64))) as u64
    }
    pub fn payload_bytes(&self) -> (usize, usize) {
        (
            self.input.len() * std::mem::size_of::<Rgba>(),
            self.output.len() * std::mem::size_of::<Rgba>(),
        )
    }
}
fn digest(pixels: &[Rgba]) -> String {
    let mut hash = Sha256::new();
    let mut scratch = [0u8; 4096 * 4];
    for chunk in pixels.chunks(4096) {
        for (p, bytes) in chunk.iter().zip(scratch.as_chunks_mut::<4>().0) {
            bytes.copy_from_slice(&p.0);
        }
        hash.update(&scratch[..chunk.len() * 4]);
    }
    paintcraft_core::types::policy::hex(&hash.finalize())
}
