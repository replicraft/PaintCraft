//! Fixed finite private float settings/report payloads, not active codec opcodes.
use super::*;
use precision::{AlphaAssignment, AlphaOutput, Curve, LinearSpace, Negative, Statistics};
pub const SETTINGS_BYTES: usize = 32;
pub const REPORT_BYTES: usize = 96;
const SETTINGS_MAGIC: &[u8; 8] = b"PCFLTS01";
const REPORT_MAGIC: &[u8; 8] = b"PCFLTR01";
fn invalid() -> Error {
    Error::new(ErrorCode::CodecFailed, "Invalid private float wire payload")
}
pub fn encode_settings(settings: Settings) -> Result<[u8; SETTINGS_BYTES]> {
    if !(-16000..=16000).contains(&settings.exposure_milli_ev) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Exposure outside private float settings allowance",
        ));
    }
    let mut out = [0; SETTINGS_BYTES];
    out[..8].copy_from_slice(SETTINGS_MAGIC);
    out[8..10].copy_from_slice(&1u16.to_be_bytes());
    out[10] = match settings.curve {
        Curve::Clip => 0,
        Curve::ReinhardPerChannel => 1,
    };
    out[11] = match settings.negative {
        Negative::Reject => 0,
        Negative::ClipToBlack => 1,
    };
    if let AlphaOutput::Flatten { srgb_background } = settings.alpha_output {
        out[12] = 1;
        out[20..23].copy_from_slice(&srgb_background);
    }
    out[13] = match settings.source_assignment {
        None => 0,
        Some(LinearSpace::Rec709D65) => 1,
        Some(LinearSpace::RadianceEqualEnergy) => 2,
    };
    out[14] = match settings.alpha_assignment {
        None => 0,
        Some(AlphaAssignment::Premultiplied) => 1,
        Some(AlphaAssignment::Straight) => 2,
    };
    out[16..20].copy_from_slice(&settings.exposure_milli_ev.to_be_bytes());
    Ok(out)
}
pub fn decode_settings(bytes: &[u8]) -> Result<Settings> {
    if bytes.len() != SETTINGS_BYTES
        || &bytes[..8] != SETTINGS_MAGIC
        || bytes[8..10] != 1u16.to_be_bytes()
    {
        return Err(invalid());
    }
    let settings = Settings {
        curve: match bytes[10] {
            0 => Curve::Clip,
            1 => Curve::ReinhardPerChannel,
            _ => return Err(invalid()),
        },
        negative: match bytes[11] {
            0 => Negative::Reject,
            1 => Negative::ClipToBlack,
            _ => return Err(invalid()),
        },
        alpha_output: match bytes[12] {
            0 => AlphaOutput::Preserve,
            1 => AlphaOutput::Flatten {
                srgb_background: bytes[20..23].try_into().unwrap(),
            },
            _ => return Err(invalid()),
        },
        source_assignment: match bytes[13] {
            0 => None,
            1 => Some(LinearSpace::Rec709D65),
            2 => Some(LinearSpace::RadianceEqualEnergy),
            _ => return Err(invalid()),
        },
        alpha_assignment: match bytes[14] {
            0 => None,
            1 => Some(AlphaAssignment::Premultiplied),
            2 => Some(AlphaAssignment::Straight),
            _ => return Err(invalid()),
        },
        exposure_milli_ev: i32::from_be_bytes(bytes[16..20].try_into().unwrap()),
    };
    if encode_settings(settings).map_err(|_| invalid())?.as_slice() != bytes {
        return Err(invalid());
    }
    Ok(settings)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub(super) digest: [u8; 32],
    pub(super) statistics: Statistics,
    pub(super) partial: u64,
}
impl Report {
    pub(super) fn from_prepared(value: &Prepared) -> Self {
        Self {
            digest: value.assessment.digest(),
            statistics: value.statistics(),
            partial: value.assessment.retained_partial_alpha_pixels(),
        }
    }
    pub fn statistics(self) -> Statistics {
        self.statistics
    }
    pub fn assessment_digest(self) -> [u8; 32] {
        self.digest
    }
    pub fn retained_partial_alpha_pixels(self) -> u64 {
        self.partial
    }
    pub fn encode(self) -> [u8; REPORT_BYTES] {
        let mut out = [0; REPORT_BYTES];
        out[..8].copy_from_slice(REPORT_MAGIC);
        out[8..10].copy_from_slice(&1u16.to_be_bytes());
        out[16..48].copy_from_slice(&self.digest);
        for (index, count) in [
            self.statistics.negative_source_samples,
            self.statistics.negative_target_samples_clipped,
            self.statistics.highlights_clipped,
            self.statistics.additive_zero_alpha_pixels,
            self.statistics.alpha_samples_quantized,
            self.partial,
        ]
        .into_iter()
        .enumerate()
        {
            out[48 + index * 8..56 + index * 8].copy_from_slice(&count.to_be_bytes());
        }
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REPORT_BYTES
            || &bytes[..8] != REPORT_MAGIC
            || bytes[8..10] != 1u16.to_be_bytes()
            || bytes[10..16] != [0; 6]
        {
            return Err(invalid());
        }
        let count =
            |i: usize| u64::from_be_bytes(bytes[48 + i * 8..56 + i * 8].try_into().unwrap());
        Ok(Self {
            digest: bytes[16..48].try_into().unwrap(),
            statistics: Statistics {
                negative_source_samples: count(0),
                negative_target_samples_clipped: count(1),
                highlights_clipped: count(2),
                additive_zero_alpha_pixels: count(3),
                alpha_samples_quantized: count(4),
            },
            partial: count(5),
        })
    }
}
#[cfg(test)]
mod tests;
