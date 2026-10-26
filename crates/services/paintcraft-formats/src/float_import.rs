//! Private float-source conversion preparation only. Digest consent binds these
//! bytes/settings/results; filesystem freshness and editor-owner publication
//! remain caller/native Job responsibilities. No worker or frontend enabled.
mod plan;
mod sources;
pub mod wire;
use crate::precision::{self, Binding, Converted, Settings};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
    policy::hex,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
pub use plan::{SourcePlan, WorkerAssessment};
use sha2::{Digest, Sha256};
pub use sources::plan_exr;
pub use sources::plan_hdr;
#[cfg(feature = "exr-codec")]
pub use sources::prepare_exr;
#[cfg(feature = "hdr-codec")]
pub use sources::prepare_hdr;
pub const KNOWN_LOSSES: &[&str] = &[
    "hdr-metadata-not-retained",
    "hdr-pixel-aspect-normalized",
    "exr-origin-normalized",
    "hdr-rgbe-lower-edge-representative",
    "hdr-linear-alpha-compositing-changed",
];
#[derive(Debug)]
struct Source {
    format: &'static str,
    len: usize,
    origin: [i32; 2],
    aspect_bits: u64,
    convention: &'static str,
    header: Vec<u8>,
    channels: Vec<(&'static str, &'static str)>,
}
#[derive(Debug)]
pub struct Assessment {
    binding: Binding,
    source: Source,
    losses: Vec<&'static str>,
    rgba_sha256: [u8; 32],
    digest: [u8; 32],
    statistics: precision::Statistics,
    retained_partial_alpha_pixels: u64,
}
impl Assessment {
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn retained_partial_alpha_pixels(&self) -> u64 {
        self.retained_partial_alpha_pixels
    }
    pub fn origin(&self) -> [i32; 2] {
        self.source.origin
    }
    pub fn pixel_aspect(&self) -> f64 {
        f64::from_bits(self.source.aspect_bits)
    }
    /// Exact original bounded header bytes, retained in this assessment only.
    /// This does not claim metadata preservation in a future PCND document.
    pub fn source_header(&self) -> &[u8] {
        &self.source.header
    }
    pub fn losses(&self) -> &[&'static str] {
        &self.losses
    }
    /// Detached JSON storage belongs to the caller's receipt admission.
    pub fn to_value(&self) -> Value {
        Value::object([
            ("policy", Value::str("float-import-preparation-v1")),
            ("conversion", self.binding.to_value()),
            (
                "statistics",
                Value::object([
                    (
                        "negative_source_samples",
                        Value::Int(self.statistics.negative_source_samples as i64),
                    ),
                    (
                        "negative_target_samples_clipped",
                        Value::Int(self.statistics.negative_target_samples_clipped as i64),
                    ),
                    (
                        "highlights_clipped",
                        Value::Int(self.statistics.highlights_clipped as i64),
                    ),
                    (
                        "additive_zero_alpha_pixels",
                        Value::Int(self.statistics.additive_zero_alpha_pixels as i64),
                    ),
                    (
                        "alpha_samples_quantized",
                        Value::Int(self.statistics.alpha_samples_quantized as i64),
                    ),
                ]),
            ),
            (
                "destination_origin",
                Value::Array(vec![Value::Int(0), Value::Int(0)]),
            ),
            (
                "destination_pixel_aspect",
                Value::str("square-1:1-without-resampling"),
            ),
            (
                "destination_blending",
                Value::str("encoded-srgb-u8-source-over"),
            ),
            (
                "retained_partial_alpha_pixels",
                Value::Int(self.retained_partial_alpha_pixels as i64),
            ),
            ("source_format", Value::str(self.source.format)),
            ("source_bytes", Value::Int(self.source.len as i64)),
            (
                "origin",
                Value::Array(
                    self.source
                        .origin
                        .into_iter()
                        .map(|v| Value::Int(i64::from(v)))
                        .collect(),
                ),
            ),
            (
                "pixel_aspect_f64_bits",
                Value::str(format!("{:016x}", self.source.aspect_bits)),
            ),
            ("sample_convention", Value::str(self.source.convention)),
            (
                "source_channels",
                Value::Array(
                    self.source
                        .channels
                        .iter()
                        .map(|(name, kind)| {
                            Value::object([
                                ("name", Value::str(*name)),
                                ("type", Value::str(*kind)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("source_header_hex", Value::str(hex(&self.source.header))),
            ("metadata_preserved_in_document", Value::Bool(false)),
            ("rgba_sha256", Value::str(hex(&self.rgba_sha256))),
            (
                "losses",
                Value::Array(self.losses.iter().copied().map(Value::str).collect()),
            ),
        ])
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if allowed.iter().any(|v| {
            !KNOWN_LOSSES.contains(&v.as_str()) && !precision::KNOWN_LOSSES.contains(&v.as_str())
        }) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown float import consent",
            ));
        }
        if self
            .losses
            .iter()
            .any(|loss| !allowed.iter().any(|v| v == loss))
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Float import requires consent for every reported limitation",
            ));
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Prepared {
    converted: Converted,
    assessment: Assessment,
    _metadata: BudgetAccount,
}
impl Prepared {
    pub fn assessment(&self) -> &Assessment {
        &self.assessment
    }
    /// Preview only; an actual consumer must adopt Approved after consent and
    /// independently revalidate the source/editor owner at publication.
    pub fn preview_rgba(&self) -> &[u8] {
        self.converted.rgba()
    }
    pub fn report(&self) -> wire::Report {
        wire::Report::from_prepared(self)
    }
    pub fn statistics(&self) -> precision::Statistics {
        self.converted.statistics()
    }
    pub fn approve(self, expected_digest: [u8; 32], allowed: &[String]) -> Result<Approved> {
        if expected_digest != self.assessment.digest() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Float import assessment/settings changed since consent",
            ));
        }
        self.assessment.require_losses(allowed)?;
        Ok(Approved { prepared: self })
    }
}
#[derive(Debug)]
pub struct Approved {
    prepared: Prepared,
}
impl Approved {
    pub fn rgba(&self) -> &[u8] {
        self.prepared.converted.rgba()
    }
    pub fn assessment(&self) -> &Assessment {
        &self.prepared.assessment
    }
}
fn scratch(budget: &SharedBudget) -> Result<BudgetAccount> {
    let mut account = budget.account(AllocationClass::PaintMetadata);
    account.resize(3 << 20)?;
    Ok(account)
}
fn source(
    format: &'static str,
    bytes: usize,
    geometry: ([i32; 2], f64),
    convention: &'static str,
    header: &[u8],
    channels: Vec<(&'static str, &'static str)>,
    budget: &SharedBudget,
) -> Result<(Source, BudgetAccount)> {
    let (origin, aspect) = geometry;
    let mut account = budget.account(AllocationClass::PaintMetadata);
    account.resize((header.len() + 4096) as u64)?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(header.len())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Float source metadata allocation"))?;
    owned.extend_from_slice(header);
    account.resize((owned.capacity() + 4096) as u64)?;
    Ok((
        Source {
            format,
            len: bytes,
            origin,
            aspect_bits: aspect.to_bits(),
            convention,
            header: owned,
            channels,
        },
        account,
    ))
}
#[cfg(any(feature = "hdr-codec", feature = "exr-codec"))]
fn finish(
    plan: SourcePlan,
    samples: &[f32],
    budget: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    let converted = precision::convert(plan.binding.clone(), samples, budget, check)?;
    let partial = if plan.retains_alpha() {
        let mut count = 0;
        for (index, pixel) in samples.as_chunks::<4>().0.iter().enumerate() {
            if index % 1024 == 0 {
                check()?;
            }
            if pixel[3] > 0.0 && pixel[3] < 1.0 {
                count += 1;
            }
        }
        count
    } else {
        0
    };
    let statistics = converted.statistics();
    let assessment = plan.assessment(statistics, partial, converted.rgba(), check)?;
    Ok(Prepared {
        converted,
        assessment: assessment.assessment,
        _metadata: assessment._metadata,
    })
}

#[cfg(all(test, feature = "hdr-codec", feature = "exr-codec"))]
mod tests;
