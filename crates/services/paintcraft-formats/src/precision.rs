//! Explicit float/HDR -> SDR policy only. No decoder, format admission or
//! frontend route is enabled here. Callers retain source/decoder admission;
//! converted storage retains its own logical output charge, not an RSS cap.
mod color;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
    policy::hex,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
use sha2::{Digest, Sha256};
pub const PIXELS_MAX: u64 = 4_194_304;
pub const POLICY_VERSION: &str = "float-to-sdr-rgba8-v1";
pub const KNOWN_LOSSES: &[&str] = &[
    "hdr-precision-reduced-to-8",
    "hdr-tone-mapped",
    "hdr-highlights-clipped",
    "hdr-negative-values-clipped",
    "hdr-source-space-assigned",
    "hdr-alpha-convention-assigned",
    "hdr-alpha-flattened",
    "hdr-alpha-quantized",
    "hdr-color-space-converted",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinearSpace {
    Rec709D65,
    RadianceEqualEnergy,
}
impl LinearSpace {
    pub fn id(self) -> &'static str {
        match self {
            Self::Rec709D65 => "linear-rec709-d65",
            Self::RadianceEqualEnergy => "linear-radiance-nominal-ee",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorEvidence {
    InteropId,
    Chromaticities,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMetadata {
    /// Neither RGB names nor an absent EXR tag establish a source color space.
    Unspecified,
    Declared {
        space: LinearSpace,
        evidence: ColorEvidence,
    },
    /// Radiance RGBE standard nominal primaries/equal-energy white, not sRGB.
    RadianceDefault,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceAlpha {
    Opaque,
    Premultiplied,
    Straight,
}
impl SourceAlpha {
    fn id(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::Premultiplied => "premultiplied",
            Self::Straight => "straight",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaAssignment {
    Premultiplied,
    Straight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Clip,
    ReinhardPerChannel,
}
impl Curve {
    pub fn id(self) -> &'static str {
        match self {
            Self::Clip => "clip-linear-0-1",
            Self::ReinhardPerChannel => "reinhard-per-channel",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Negative {
    Reject,
    ClipToBlack,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaOutput {
    Preserve,
    /// Chosen sRGB8 backdrop is decoded to destination-linear Rec.709 and
    /// composed there BEFORE the selected exposure/curve. This is explicit
    /// flattening, including premultiplied zero-alpha additive color.
    Flatten {
        srgb_background: [u8; 3],
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub curve: Curve,
    pub exposure_milli_ev: i32,
    pub negative: Negative,
    pub alpha_output: AlphaOutput,
    pub source_assignment: Option<LinearSpace>,
    pub alpha_assignment: Option<AlphaAssignment>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            curve: Curve::Clip,
            exposure_milli_ev: 0,
            negative: Negative::Reject,
            alpha_output: AlphaOutput::Preserve,
            source_assignment: None,
            alpha_assignment: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    source_sha256: [u8; 32],
    width: u32,
    height: u32,
    metadata: ColorMetadata,
    source_alpha: SourceAlpha,
    settings: Settings,
    space: LinearSpace,
    alpha: SourceAlpha,
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
impl Binding {
    /// The format owner must derive metadata/alpha conventions from the source
    /// and supply its captured digest. This is a policy binding, not a decoder
    /// or proof that arbitrary caller-supplied metadata describes those bytes.
    pub fn new(
        source_sha256: [u8; 32],
        width: u32,
        height: u32,
        metadata: ColorMetadata,
        source_alpha: SourceAlpha,
        settings: Settings,
    ) -> Result<Self> {
        if width == 0
            || height == 0
            || width > 65536
            || height > 65536
            || u64::from(width) * u64::from(height) > PIXELS_MAX
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "SDR conversion dimensions exceed allowance",
            ));
        }
        if !(-16000..=16000).contains(&settings.exposure_milli_ev) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Exposure must be between -16 and +16 EV",
            ));
        }
        let declared = match metadata {
            ColorMetadata::Unspecified => None,
            ColorMetadata::Declared { space, .. } => Some(space),
            ColorMetadata::RadianceDefault => Some(LinearSpace::RadianceEqualEnergy),
        };
        let space = settings.source_assignment.or(declared).ok_or_else(|| {
            unsupported("Choose the image's source color space before conversion")
        })?;
        if source_alpha == SourceAlpha::Opaque && settings.alpha_assignment.is_some() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "An opaque source has no alpha convention to assign",
            ));
        }
        let alpha = match settings.alpha_assignment {
            None => source_alpha,
            Some(AlphaAssignment::Premultiplied) => SourceAlpha::Premultiplied,
            Some(AlphaAssignment::Straight) => SourceAlpha::Straight,
        };
        Ok(Self {
            source_sha256,
            width,
            height,
            metadata,
            source_alpha,
            settings,
            space,
            alpha,
        })
    }
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn settings(&self) -> Settings {
        self.settings
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    pub fn to_value(&self) -> Value {
        let (metadata_space, evidence) = match self.metadata {
            ColorMetadata::Unspecified => (Value::Null, "unresolved"),
            ColorMetadata::RadianceDefault => (
                Value::str(LinearSpace::RadianceEqualEnergy.id()),
                "radiance-normative-default",
            ),
            ColorMetadata::Declared { space, evidence } => (
                Value::str(space.id()),
                match evidence {
                    ColorEvidence::InteropId => "explicit-color-interop-id",
                    ColorEvidence::Chromaticities => "explicit-chromaticities",
                },
            ),
        };
        let output = match self.settings.alpha_output {
            AlphaOutput::Preserve => Value::object([("mode", Value::str("preserve"))]),
            AlphaOutput::Flatten { srgb_background } => Value::object([
                ("mode", Value::str("flatten-before-exposure-and-curve")),
                (
                    "srgb_background",
                    Value::Array(
                        srgb_background
                            .into_iter()
                            .map(|c| Value::Int(i64::from(c)))
                            .collect(),
                    ),
                ),
            ]),
        };
        Value::object([
            ("policy", Value::str(POLICY_VERSION)),
            ("source_sha256", Value::str(hex(&self.source_sha256))),
            ("width", Value::Int(i64::from(self.width))),
            ("height", Value::Int(i64::from(self.height))),
            ("metadata_color_space", metadata_space),
            ("color_evidence", Value::str(evidence)),
            ("resolved_linear_space", Value::str(self.space.id())),
            (
                "source_space_assigned",
                Value::Bool(self.settings.source_assignment.is_some()),
            ),
            ("source_alpha", Value::str(self.source_alpha.id())),
            ("effective_alpha", Value::str(self.alpha.id())),
            (
                "alpha_assigned",
                Value::Bool(self.settings.alpha_assignment.is_some()),
            ),
            ("alpha_output", output),
            ("curve", Value::str(self.settings.curve.id())),
            (
                "exposure_milli_ev",
                Value::Int(i64::from(self.settings.exposure_milli_ev)),
            ),
            (
                "negative_values",
                Value::str(match self.settings.negative {
                    Negative::Reject => "reject",
                    Negative::ClipToBlack => "clip-to-black",
                }),
            ),
            ("nonfinite_values", Value::str("reject")),
            ("out_of_range_alpha", Value::str("reject")),
            ("destination", Value::str("straight-srgb-rgba8")),
        ])
    }
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"PaintCraft SDR binding v1\0");
        hash.update(json::canonical(&self.to_value()));
        hash.finalize().into()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    pub negative_source_samples: u64,
    pub negative_target_samples_clipped: u64,
    pub highlights_clipped: u64,
    pub additive_zero_alpha_pixels: u64,
    pub alpha_samples_quantized: u64,
}
#[derive(Debug)]
pub struct Converted {
    rgba: Vec<u8>,
    binding: Binding,
    statistics: Statistics,
    losses: Vec<&'static str>,
    // Storage drops before its retained logical declaration.
    _account: BudgetAccount,
}
impl Converted {
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
    pub fn losses(&self) -> &[&'static str] {
        &self.losses
    }
    pub fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if allowed
            .iter()
            .any(|code| !KNOWN_LOSSES.contains(&code.as_str()))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown SDR conversion loss consent",
            ));
        }
        if self
            .losses
            .iter()
            .any(|loss| !allowed.iter().any(|code| code == loss))
        {
            return Err(unsupported(
                "SDR conversion requires consent for all reported limitations",
            ));
        }
        Ok(())
    }
}
/// Source storage/decoder scratch remains the caller's admitted responsibility.
/// Exactly RGB float32 for opaque, RGBA float32 otherwise. No partial result can
/// escape a cancellation, invalid sample or denied output allocation.
pub fn convert(
    binding: Binding,
    samples: &[f32],
    output_budget: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    check()?;
    let count = u64::from(binding.width) * u64::from(binding.height);
    let channels = if binding.source_alpha == SourceAlpha::Opaque {
        3
    } else {
        4
    };
    if samples.len() as u64 != count * channels as u64 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Float sample count disagrees with captured image",
        ));
    }
    let output_bytes = count * 4;
    let mut account = output_budget.account(AllocationClass::DecodedTiles);
    account.resize(output_bytes)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(output_bytes as usize)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "SDR output allocation failed"))?;
    account.resize(rgba.capacity() as u64)?;
    let exposure = 2.0_f64.powf(f64::from(binding.settings.exposure_milli_ev) / 1000.0);
    let mut statistics = Statistics::default();
    for (index, sample) in samples.chunks_exact(channels).enumerate() {
        if index % 1024 == 0 {
            check()?;
        }
        if sample.iter().any(|v| !v.is_finite()) {
            return Err(unsupported(
                "Nonfinite float color or alpha cannot be converted",
            ));
        }
        let alpha = if channels == 4 {
            f64::from(sample[3])
        } else {
            1.0
        };
        if !(0.0..=1.0).contains(&alpha) {
            return Err(unsupported(
                "Float alpha must be finite and between zero and one",
            ));
        }
        let source = [
            f64::from(sample[0]),
            f64::from(sample[1]),
            f64::from(sample[2]),
        ];
        for v in source {
            if v < 0.0 {
                statistics.negative_source_samples += 1;
                if binding.settings.negative == Negative::Reject {
                    return Err(unsupported(
                        "Negative source values require an explicit clipping policy",
                    ));
                }
            }
        }
        let mut linear = color::to_rec709(binding.space, source);
        let output_alpha = match binding.settings.alpha_output {
            AlphaOutput::Preserve => {
                if binding.alpha == SourceAlpha::Premultiplied {
                    if alpha == 0.0 {
                        if source != [0.0; 3] {
                            return Err(unsupported(
                                "Zero-alpha additive color requires an opaque background",
                            ));
                        }
                        linear = [0.0; 3];
                    } else {
                        linear = linear.map(|v| v / alpha);
                    }
                }
                alpha
            }
            AlphaOutput::Flatten { srgb_background } => {
                let background = color::background(srgb_background);
                if binding.alpha == SourceAlpha::Premultiplied && alpha == 0.0 && source != [0.0; 3]
                {
                    statistics.additive_zero_alpha_pixels += 1;
                }
                for c in 0..3 {
                    linear[c] = (if binding.alpha == SourceAlpha::Straight {
                        linear[c] * alpha
                    } else {
                        linear[c]
                    }) + background[c] * (1.0 - alpha);
                }
                1.0
            }
        };
        for value in linear {
            let mut value = value * exposure;
            if !value.is_finite() {
                return Err(unsupported("Nonfinite color conversion result"));
            }
            if value < 0.0 {
                if binding.settings.negative == Negative::Reject {
                    return Err(unsupported(
                        "Out-of-gamut negative color requires an explicit clipping policy",
                    ));
                }
                statistics.negative_target_samples_clipped += 1;
                value = 0.0;
            }
            let mapped = match binding.settings.curve {
                Curve::Clip => {
                    if value > 1.0 {
                        statistics.highlights_clipped += 1;
                    }
                    value.min(1.0)
                }
                Curve::ReinhardPerChannel => value / (1.0 + value),
            };
            rgba.push(color::encode(mapped));
        }
        let byte = (output_alpha * 255.0 + 0.5).floor() as u8;
        if output_alpha != f64::from(byte) / 255.0 {
            statistics.alpha_samples_quantized += 1;
        }
        rgba.push(byte);
    }
    check()?;
    let mut losses = vec![KNOWN_LOSSES[0]];
    if binding.settings.curve == Curve::ReinhardPerChannel {
        losses.push("hdr-tone-mapped");
    }
    if statistics.highlights_clipped > 0 {
        losses.push("hdr-highlights-clipped");
    }
    if statistics.negative_target_samples_clipped > 0 {
        losses.push("hdr-negative-values-clipped");
    }
    if binding.settings.source_assignment.is_some() {
        losses.push("hdr-source-space-assigned");
    }
    if binding.settings.alpha_assignment.is_some() {
        losses.push("hdr-alpha-convention-assigned");
    }
    if binding.settings.alpha_output != AlphaOutput::Preserve
        && binding.source_alpha != SourceAlpha::Opaque
    {
        losses.push("hdr-alpha-flattened");
    }
    if statistics.alpha_samples_quantized > 0 {
        losses.push("hdr-alpha-quantized");
    }
    if binding.space != LinearSpace::Rec709D65 {
        losses.push("hdr-color-space-converted");
    }
    Ok(Converted {
        rgba,
        binding,
        statistics,
        losses,
        _account: account,
    })
}

#[cfg(test)]
mod tests;
