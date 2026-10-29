//! Shared pure-Rust container/typed-raster entrypoints for replay and libFuzzer.
//! Declared parser limits are not a hard RSS limit or a native-code sandbox.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::cell::Cell;
mod pcnd;
mod stock;
pub const INPUT_MAX: usize = 256 << 10;
pub const PIXELS_MAX: u64 = 16_384;
pub const CHECKPOINT_MAX: u32 = 4096;
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Avif,
    Heif,
    Paint,
    Pdn,
    Dcx,
    Radiance,
    Exr,
    Raster,
    TiffPages,
    Icons,
    Pcnd,
}
impl Target {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "avif" => Self::Avif,
            "heif" => Self::Heif,
            "paint" => Self::Paint,
            "pdn" => Self::Pdn,
            "dcx" => Self::Dcx,
            "radiance" => Self::Radiance,
            "exr" => Self::Exr,
            "raster" => Self::Raster,
            "tiff_pages" => Self::TiffPages,
            "icons" => Self::Icons,
            "pcnd" => Self::Pcnd,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Rejected,
}
/// Parser invariant failures are findings in both replay and libFuzzer. Other
/// returned errors remain rejections; this does not classify each as harmless.
pub fn classify(result: Result<()>) -> Outcome {
    match result {
        Ok(()) => Outcome::Accepted,
        Err(error) if error.code == ErrorCode::InvariantViolation => {
            panic!("parser internal invariant failed: {error}")
        }
        Err(_) => Outcome::Rejected,
    }
}

/// Decoder C code and editor publication are deliberately outside these targets.
/// Entry wrappers classify returned errors; invariant failures and panics remain
/// findings rather than ordinary rejections.
pub fn exercise(target: Target, bytes: &[u8]) -> Result<()> {
    exercise_mode(target, bytes, false)
}
/// Harness-only semantic mutation route. Repairs only the outer PCND checksum;
/// the product parser still validates every inner digest and encoded payload.
pub fn exercise_pcnd_checksum_aware(bytes: &[u8]) -> Result<()> {
    exercise_mode(Target::Pcnd, bytes, true)
}
fn exercise_mode(target: Target, bytes: &[u8], checksum_aware: bool) -> Result<()> {
    if bytes.len() > INPUT_MAX {
        return Err(Error::new(ErrorCode::ResourceLimit, "fuzz input allowance"));
    }
    let checkpoints = Cell::new(0u32);
    let check = || {
        let next = checkpoints.get().saturating_add(1);
        checkpoints.set(next);
        if next > CHECKPOINT_MAX {
            Err(Error::new(
                ErrorCode::ResourceLimit,
                "fuzz checkpoint allowance",
            ))
        } else {
            Ok(())
        }
    };
    let repaired;
    let bytes = if checksum_aware {
        repaired = pcnd::repair_outer_sha(bytes, &check)?;
        repaired.as_slice()
    } else {
        bytes
    };
    match target {
        Target::Pcnd => pcnd::exercise(bytes, &check)?,
        Target::Avif => {
            paintcraft_formats::avif::assess_family(bytes, PIXELS_MAX, &check)?;
        }
        Target::Heif => {
            paintcraft_formats::heif::inspect_checked(bytes, PIXELS_MAX, &check)?;
        }
        Target::Radiance => {
            paintcraft_formats::radiance::inspect(bytes, PIXELS_MAX, &check)?;
        }
        Target::Exr => {
            paintcraft_formats::exr::inspect(bytes, PIXELS_MAX, &check)?;
        }
        Target::Raster | Target::TiffPages | Target::Icons => {
            stock::exercise(target, bytes, &check)?;
        }
        Target::Paint => {
            let limits = paintcraft_paint::DecodeLimits {
                container: paintcraft_paint::Limits {
                    source_bytes: INPUT_MAX,
                    boxes: 512,
                    depth: 8,
                    metadata_bytes: 64 << 10,
                    compatible_brands: 32,
                    extents: 512,
                },
                items: 128,
                layers: 16,
                dimension: 4096,
                pixels: PIXELS_MAX,
                decoded_bytes: PIXELS_MAX as usize * 4,
            };
            paintcraft_paint::decode_layers(bytes, &limits, &check)?;
        }
        Target::Pdn => {
            let limits = paintcraft_pdn::DecodeLimits {
                envelope: paintcraft_pdn::Limits {
                    source_bytes: INPUT_MAX,
                    header_bytes: 64 << 10,
                    xml_depth: 16,
                    xml_elements: 512,
                    attributes_per_element: 32,
                    attribute_bytes: 16 << 10,
                    dimension: 4096,
                    pixels: PIXELS_MAX,
                    layers: 16,
                },
                graph_bytes: 128 << 10,
                graph_metadata_bytes: 128 << 10,
                objects: 512,
                array_items: 4096,
                members: 64,
                object_depth: 16,
                decoded_pixels: PIXELS_MAX,
                decoded_bytes: PIXELS_MAX as usize * 4,
                chunks: 512,
            };
            paintcraft_pdn::decode_layers(bytes, &limits, &check)?;
        }
        Target::Dcx => {
            let pages = paintcraft_formats::dcx::inspect(bytes)?;
            paintcraft_formats::dcx::decode_page(bytes, 0, PIXELS_MAX)?;
            if pages.len() > 1 {
                check()?;
                paintcraft_formats::dcx::decode_page(bytes, (pages.len() - 1) as u16, PIXELS_MAX)?;
            }
        }
    }
    check()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invariant_failures_are_not_counted_as_ordinary_rejections() {
        assert_eq!(classify(Ok(())), Outcome::Accepted);
        for code in [
            ErrorCode::CodecFailed,
            ErrorCode::CapabilityUnsupported,
            ErrorCode::ResourceLimit,
        ] {
            assert_eq!(
                classify(Err(Error::new(code, "expected control"))),
                Outcome::Rejected
            );
        }
        let failed = std::panic::catch_unwind(|| {
            classify(Err(Error::new(
                ErrorCode::InvariantViolation,
                "internal control",
            )))
        });
        assert!(failed.is_err());
    }
    #[test]
    fn actual_dcx_entrypoint_retains_positive_and_negative_controls() {
        let source = include_bytes!(
            "../../crates/services/paintcraft-formats/tests/dcx-fixtures/writer/rgb.dcx"
        );
        assert_eq!(classify(exercise(Target::Dcx, source)), Outcome::Accepted);
        assert_eq!(classify(exercise(Target::Dcx, &[])), Outcome::Rejected);
    }
    const HDR: &[u8] = include_bytes!(
        "../../crates/services/paintcraft-formats/tests/radiance-fixtures/modern-rle-8x2.hdr"
    );
    const EXR: &[u8] = include_bytes!(
        "../../crates/services/paintcraft-formats/tests/exr-fixtures/mixed-rgba-zip.exr"
    );
    #[test]
    fn float_inspectors_keep_independent_positive_and_malformed_controls() {
        assert!(matches!(Target::parse("radiance"), Some(Target::Radiance)));
        assert!(matches!(Target::parse("exr"), Some(Target::Exr)));
        for (target, source) in [(Target::Radiance, HDR), (Target::Exr, EXR)] {
            assert_eq!(classify(exercise(target, source)), Outcome::Accepted);
            for length in [0, 1, source.len() - 1] {
                assert_eq!(
                    classify(exercise(target, &source[..length])),
                    Outcome::Rejected
                );
            }
            let oversized = vec![0; INPUT_MAX + 1];
            assert_eq!(
                exercise(target, &oversized).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
        }
    }
    #[test]
    fn float_inspector_pixel_limits_are_not_bypassed() {
        let wide_hdr = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 16385\n";
        assert_eq!(
            exercise(Target::Radiance, wide_hdr).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        // Change only the two trusted fixture windows, preserving their equality.
        // The declared width now exceeds the harness pixel cap before chunk work.
        let inspected = paintcraft_formats::exr::inspect(EXR, PIXELS_MAX, &|| Ok(())).unwrap();
        let mut wide_exr = EXR.to_vec();
        let mut windows = 0;
        for attribute in &inspected.header().attributes {
            if matches!(attribute.name, "dataWindow" | "displayWindow") {
                let start = attribute.value.as_ptr() as usize - EXR.as_ptr() as usize;
                let xmin = i32::from_le_bytes(attribute.value[..4].try_into().unwrap());
                wide_exr[start + 8..start + 12]
                    .copy_from_slice(&(xmin + PIXELS_MAX as i32).to_le_bytes());
                windows += 1;
            }
        }
        assert_eq!(windows, 2);
        assert_eq!(
            exercise(Target::Exr, &wide_exr).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    #[test]
    fn callback_allowance_rejects_a_valid_but_work_heavy_radiance_inventory() {
        let mut many_rows = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 8192 +X 1\n".to_vec();
        many_rows.resize(many_rows.len() + 8192 * 4, 0);
        assert!(many_rows.len() <= INPUT_MAX);
        assert!(paintcraft_formats::radiance::inspect(&many_rows, PIXELS_MAX, &|| Ok(())).is_ok());
        assert_eq!(
            exercise(Target::Radiance, &many_rows).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}
