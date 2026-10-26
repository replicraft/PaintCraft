//! Shared pure-Rust container/typed-raster entrypoints for replay and libFuzzer.
//! Declared parser limits are not a hard RSS limit or a native-code sandbox.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::cell::Cell;
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
}
impl Target {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "avif" => Self::Avif,
            "heif" => Self::Heif,
            "paint" => Self::Paint,
            "pdn" => Self::Pdn,
            "dcx" => Self::Dcx,
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
    match target {
        Target::Avif => {
            paintcraft_formats::avif::assess_family(bytes, PIXELS_MAX, &check)?;
        }
        Target::Heif => {
            paintcraft_formats::heif::inspect_checked(bytes, PIXELS_MAX, &check)?;
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
}
