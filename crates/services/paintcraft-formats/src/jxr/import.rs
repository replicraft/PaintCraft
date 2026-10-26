//! Exact source assessment for the admitted native RGB8 conversion profile.
use super::*;
pub const KNOWN_IMPORT_LOSSES: &[&str] = &["jxr-resolution-not-retained"];
/// T.832 A.7.25/26: omitted or numeric zero resolution infers 96 pixels/inch.
/// Raw source bits are retained independently; no header value is rewritten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub raw_bits: Option<u32>,
    pub effective_bits: u32,
    pub inferred_96: bool,
}
impl Resolution {
    fn from_bits(raw_bits: Option<u32>) -> Self {
        let inferred_96 = raw_bits.is_none_or(|bits| f32::from_bits(bits) == 0.0);
        Self {
            raw_bits,
            effective_bits: if inferred_96 {
                96f32.to_bits()
            } else {
                raw_bits.unwrap()
            },
            inferred_96,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportAssessment {
    source: Inspection,
    resolution_x: Resolution,
    resolution_y: Resolution,
}
impl ImportAssessment {
    pub fn source(&self) -> Inspection {
        self.source
    }
    pub fn resolution_x(&self) -> Resolution {
        self.resolution_x
    }
    pub fn resolution_y(&self) -> Resolution {
        self.resolution_y
    }
    pub fn color_policy(&self) -> &'static str {
        COLOR_POLICY
    }
    /// Flat PaintCraft documents do not carry source physical pixel density,
    /// including the normative 96 dpi inferred for absent/zero source fields.
    pub fn losses(&self) -> &'static [&'static str] {
        KNOWN_IMPORT_LOSSES
    }
    /// Stable receipt values distinguish encoded bits from normative inference.
    pub fn to_value(&self) -> paintcraft_core::types::json::Value {
        use paintcraft_core::types::json::Value;
        fn resolution(value: Resolution) -> Value {
            Value::object([
                (
                    "raw_f32_bits",
                    value
                        .raw_bits
                        .map_or(Value::Null, |bits| Value::str(format!("{bits:08x}"))),
                ),
                (
                    "effective_f32_bits",
                    Value::str(format!("{:08x}", value.effective_bits)),
                ),
                ("inferred_96_dpi", Value::Bool(value.inferred_96)),
            ])
        }
        Value::object([
            ("color_policy", Value::str(self.color_policy())),
            ("resolution_x", resolution(self.resolution_x())),
            ("resolution_y", resolution(self.resolution_y())),
            (
                "losses",
                Value::Array(self.losses().iter().copied().map(Value::str).collect()),
            ),
            ("preserving_save", Value::Bool(false)),
        ])
    }
    pub fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if let Some(code) = allowed
            .iter()
            .find(|code| !KNOWN_IMPORT_LOSSES.contains(&code.as_str()))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                format!("Unknown JXR import loss {code:?}"),
            ));
        }
        if !allowed.iter().any(|code| code == KNOWN_IMPORT_LOSSES[0]) {
            return Err(unsupported(
                "JXR import requires jxr-resolution-not-retained consent",
            ));
        }
        Ok(())
    }
}
pub fn assess_import(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<ImportAssessment> {
    let source = inspect_checked(bytes, pixels, check)?;
    Ok(ImportAssessment {
        source,
        resolution_x: Resolution::from_bits(source.resolution_x_bits),
        resolution_y: Resolution::from_bits(source.resolution_y_bits),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &[u8] = include_bytes!("../../tests/jxr-fixtures/rgb8-4x3.jxr");
    fn resolution(bytes: &mut [u8], tag: u16, bits: u32) {
        let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
        let pos = (0..count)
            .map(|index| ifd + 2 + index * 12)
            .find(|pos| u16::from_le_bytes(bytes[*pos..*pos + 2].try_into().unwrap()) == tag)
            .unwrap();
        bytes[pos + 8..pos + 12].copy_from_slice(&bits.to_le_bytes());
    }
    #[test]
    fn raw_and_normative_effective_density_are_distinct_and_always_require_actual_loss_consent() {
        // Primary reference: ITU-T T.832(2012) A.7.25/26, printed page 172.
        // https://www.itu.int/rec/T-REC-T.832-201201-I/en
        let source = SOURCE.to_vec();
        let original = source.clone();
        let assessment = assess_import(&source, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            assessment.resolution_x(),
            Resolution {
                raw_bits: Some(0),
                effective_bits: 96f32.to_bits(),
                inferred_96: true
            }
        );
        assert_eq!(assessment.color_policy(), COLOR_POLICY);
        assert!(assessment.require_losses(&[]).is_err());
        assert!(
            assessment
                .require_losses(&["jxr-resolution-not-retained".into()])
                .is_ok()
        );
        assert_eq!(
            assessment
                .require_losses(&["exif-metadata-ignored".into()])
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(source, original);
        for bits in [None, Some(0), Some((-0f32).to_bits())] {
            assert!(Resolution::from_bits(bits).inferred_96);
            assert_eq!(Resolution::from_bits(bits).effective_bits, 96f32.to_bits());
        }
        let positive = include_bytes!("../../tests/jxr-fixtures/rgb8-dpi96x144.jxr");
        let positive_assessment = assess_import(positive, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            positive_assessment.resolution_x(),
            Resolution {
                raw_bits: Some(96f32.to_bits()),
                effective_bits: 96f32.to_bits(),
                inferred_96: false
            }
        );
        assert_eq!(
            positive_assessment.resolution_y().effective_bits,
            144f32.to_bits()
        );
        assert!(!positive_assessment.resolution_y().inferred_96);
        assert!(positive_assessment.require_losses(&[]).is_err());
    }
    #[test]
    fn invalid_density_unknown_color_metadata_and_cancel_do_not_create_assessment() {
        for bits in [
            f32::NAN.to_bits(),
            f32::INFINITY.to_bits(),
            (-1f32).to_bits(),
        ] {
            let mut bytes = SOURCE.to_vec();
            resolution(&mut bytes, 0xbc82, bits);
            assert!(assess_import(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
        }
        let mut bytes = SOURCE.to_vec();
        let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        bytes[ifd + 2..ifd + 4].copy_from_slice(&0x8773u16.to_le_bytes());
        assert!(assess_import(&bytes, PIXELS_MAX, &|| Ok(())).is_err());
        assert_eq!(
            assess_import(SOURCE, PIXELS_MAX, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel"
            )))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
}
