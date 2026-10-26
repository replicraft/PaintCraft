//! Bounded declaration assessment in the parent; never invokes the SPS decoder.
use super::*;
use crate::codec_worker::NativeEncoding;
use paintcraft_core::types::{ErrorCode, json::Value, policy::hex};
use sha2::{Digest, Sha256};
pub const KNOWN_IMPORT_LOSSES: &[&str] =
    &["heif-color-reconstruction", "heif-item-names-not-retained"];
pub const COLOR_POLICY: &str = "heif-explicit-srgb8-bilinear-type0-v1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportAssessment {
    width: u32,
    height: u32,
    chroma: u8,
    color: NativeEncoding,
    primary_id: u32,
    primary_name: String,
    alpha: Option<(u32, String)>,
    sha256: [u8; 32],
}
impl ImportAssessment {
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn chroma(&self) -> u8 {
        self.chroma
    }
    pub fn color(&self) -> NativeEncoding {
        self.color
    }
    pub fn alpha(&self) -> bool {
        self.alpha.is_some()
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn losses(&self) -> &'static [&'static str] {
        let names = !self.primary_name.is_empty()
            || self.alpha.as_ref().is_some_and(|value| !value.1.is_empty());
        match (self.color.matrix == 1, names) {
            (true, true) => KNOWN_IMPORT_LOSSES,
            (true, false) => &["heif-color-reconstruction"],
            (false, true) => &["heif-item-names-not-retained"],
            (false, false) => &[],
        }
    }
    pub fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if allowed
            .iter()
            .any(|code| !KNOWN_IMPORT_LOSSES.contains(&code.as_str()))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown HEIF import loss consent",
            ));
        }
        if self
            .losses()
            .iter()
            .any(|code| !allowed.iter().any(|given| given == code))
        {
            return Err(unsupported(
                "HEIF conversion requires consent for all reported import limitations",
            ));
        }
        Ok(())
    }
    pub fn to_value(&self) -> Value {
        Value::object([
            ("source_sha256", Value::str(hex(&self.sha256))),
            ("width", Value::Int(i64::from(self.width))),
            ("height", Value::Int(i64::from(self.height))),
            ("primary_item_id", Value::Int(i64::from(self.primary_id))),
            ("primary_item_name", Value::str(&self.primary_name)),
            (
                "alpha_item_id",
                self.alpha
                    .as_ref()
                    .map_or(Value::Null, |v| Value::Int(i64::from(v.0))),
            ),
            (
                "alpha_item_name",
                self.alpha
                    .as_ref()
                    .map_or(Value::Null, |v| Value::str(&v.1)),
            ),
            ("straight_alpha", Value::Bool(self.alpha())),
            ("color_policy", Value::str(COLOR_POLICY)),
            (
                "source_primaries",
                Value::Int(i64::from(self.color.primaries)),
            ),
            (
                "source_transfer",
                Value::Int(i64::from(self.color.transfer)),
            ),
            ("source_matrix", Value::Int(i64::from(self.color.matrix))),
            ("source_full_range", Value::Bool(self.color.full_range)),
            ("source_chroma_format", Value::Int(i64::from(self.chroma))),
            ("sample_depth", Value::Int(8)),
            (
                "reconstruction",
                Value::str(if self.color.matrix == 0 {
                    "identity-gbr-reordering"
                } else if self.chroma == 1 {
                    "matrix1-range-expansion-type0-bilinear-edge-extension-round-clip-rgb8"
                } else {
                    "matrix1-range-expansion-round-clip-rgb8"
                }),
            ),
            (
                "losses",
                Value::Array(self.losses().iter().copied().map(Value::str).collect()),
            ),
            ("preserving_save", Value::Bool(false)),
        ])
    }
}
fn name(value: &str) -> Result<String> {
    let mut out = String::new();
    out.try_reserve_exact(value.len())
        .map_err(|_| resource("HEIF source name allocation"))?;
    out.push_str(value);
    Ok(out)
}
fn item(item: &Item<'_>, alpha: bool) -> Result<((u32, u32), u8, NativeEncoding)> {
    if item.kind != *b"hvc1" {
        return Err(unsupported("HEIF conversion requires directly coded items"));
    }
    let mut size = None;
    let mut config = None;
    let mut color = None;
    let mut pixi = false;
    for association in &item.properties {
        match association.property {
            Property::SpatialExtent { width, height } => {
                if size.replace((width, height)).is_some() {
                    return Err(invalid("duplicate HEIF extent"));
                }
            }
            Property::HevcConfig {
                chroma,
                luma_bits: 8,
                chroma_bits: 8,
                ..
            } if if alpha {
                chroma == 0
            } else {
                matches!(chroma, 1 | 3)
            } =>
            {
                if config.replace(chroma).is_some() {
                    return Err(invalid("duplicate HEIF configuration"));
                }
            }
            Property::Color(Color::Nclx {
                primaries,
                transfer,
                matrix,
                full_range,
            }) => {
                if color
                    .replace(NativeEncoding {
                        primaries,
                        transfer,
                        matrix,
                        full_range,
                    })
                    .is_some()
                {
                    return Err(invalid("duplicate HEIF color"));
                }
            }
            Property::PixelInformation { bits }
                if bits == if alpha { &[8][..] } else { &[8, 8, 8][..] } =>
            {
                if pixi {
                    return Err(invalid("duplicate HEIF pixel information"));
                }
                pixi = true;
            }
            Property::Auxiliary {
                kind: "urn:mpeg:hevc:2015:auxid:1",
                subtype: [],
            } if alpha => {}
            Property::Rotation {
                quarter_turns_ccw: 0,
            } => {}
            _ => {
                return Err(unsupported(
                    "HEIF conversion refuses unqualified metadata/properties",
                ));
            }
        }
    }
    let size = size.ok_or_else(|| invalid("HEIF extent missing"))?;
    let chroma = config.ok_or_else(|| unsupported("HEIF8 configuration missing or unsupported"))?;
    let color = color.ok_or_else(|| unsupported("HEIF explicit NCLX missing"))?;
    if !pixi {
        return Err(unsupported(
            "HEIF conversion requires explicit unsigned8 channel information",
        ));
    }
    if alpha {
        if !color.full_range
            || !matches!(color.primaries, 1 | 2)
            || !matches!(color.transfer, 2 | 13)
            || !matches!(color.matrix, 0..=2)
        {
            return Err(unsupported("HEIF alpha declarations unsupported"));
        }
    } else if color.primaries != 1
        || color.transfer != 13
        || !matches!(color.matrix, 0 | 1)
        || (color.matrix == 0 && (!color.full_range || chroma != 3))
        || (chroma == 1 && (size.0 % 2 != 0 || size.1 % 2 != 0))
    {
        return Err(unsupported(
            "HEIF explicit sRGB8 source declarations unsupported",
        ));
    }
    Ok((size, chroma, color))
}
/// Container declarations alone do not prove coded SPS or entropy agreement.
/// The selected decoder is still confined to the isolated helper, whose typed
/// response is validated before this assessment can be used for adoption.
pub fn assess_import(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<ImportAssessment> {
    let value = inspect_checked(bytes, pixels, check)?;
    let primary = value
        .primary()
        .ok_or_else(|| invalid("HEIF primary missing"))?;
    let alpha = value.alpha();
    if value.premultiplied_alpha() || value.references.iter().any(|link| link.kind != *b"auxl") {
        return Err(unsupported(
            "HEIF conversion graph/premultiplication unsupported",
        ));
    }
    let (size, chroma, color) = item(primary, false)?;
    for candidate in &value.items {
        check()?;
        if candidate.id != primary.id && alpha.is_none_or(|alpha| alpha.id != candidate.id) {
            return Err(unsupported(
                "HEIF conversion refuses extra image/metadata items",
            ));
        }
    }
    let alpha = alpha
        .map(|alpha| {
            let (extent, _, _) = item(alpha, true)?;
            if extent != size {
                return Err(unsupported("HEIF alpha geometry differs from primary"));
            }
            Ok((alpha.id, name(alpha.name)?))
        })
        .transpose()?;
    check()?;
    Ok(ImportAssessment {
        width: size.0,
        height: size.1,
        chroma,
        color,
        primary_id: primary.id,
        primary_name: name(primary.name)?,
        alpha,
        sha256: Sha256::digest(bytes).into(),
    })
}
#[cfg(test)]
mod tests;
