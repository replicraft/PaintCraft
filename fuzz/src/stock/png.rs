//! Exercise the retained-profile path in addition to ordinary PNG/paste decoding.
use super::{invariant, samples};
use crate::PIXELS_MAX;
use paintcraft_core::types::{Result, resources::SharedBudget};
use paintcraft_formats::{image, pcnd};
use sha2::{Digest, Sha256};

const METADATA_BYTES: u64 = 16 << 10;
pub(super) fn exercise(
    bytes: &[u8],
    legacy: &image::Decoded,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    with_budget(bytes, legacy, &SharedBudget::new(METADATA_BYTES), check)
}
fn with_budget(
    bytes: &[u8],
    legacy: &image::Decoded,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let result = image::png_retention::decode(
        bytes,
        PIXELS_MAX,
        pcnd::MetadataAdmission {
            bytes: METADATA_BYTES,
            budget,
        },
        check,
    );
    let outcome = match result {
        Ok(prepared) => {
            samples(
                prepared.image.width,
                prepared.image.height,
                &prepared.image.rgba,
            )?;
            if prepared.image.rgba != legacy.rgba
                || (prepared.image.width, prepared.image.height) != (legacy.width, legacy.height)
                || prepared
                    .image
                    .losses
                    .iter()
                    .any(|loss| !image::KNOWN_IMPORT_LOSSES.contains(&loss.as_str()))
            {
                return Err(invariant(
                    "retained PNG changed samples or emitted unknown notices",
                ));
            }
            if let Some(metadata) = &prepared.attachment {
                if metadata.descriptor().source_sha256 != <[u8; 32]>::from(Sha256::digest(bytes))
                    || metadata.descriptor().source_bytes != bytes.len() as u64
                    || metadata.descriptor().source_dimensions != (legacy.width, legacy.height)
                    || metadata.descriptor().output_dimensions != (legacy.width, legacy.height)
                    || metadata.storage_usage().1 != budget.usage().reserved
                {
                    return Err(invariant(
                        "retained PNG source identity or admission charge",
                    ));
                }
            } else if budget.usage().reserved != 0 {
                return Err(invariant("metadata-free PNG retained a profile charge"));
            }
            Ok(())
        }
        Err(error) => Err(error),
    };
    if budget.usage().reserved != 0 {
        return Err(invariant(
            "retained PNG success/refusal leaked metadata admission",
        ));
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{Error, ErrorCode};
    #[test]
    fn real_profile_capture_releases_on_success_quota_refusal_and_cancellation() {
        let bytes = include_bytes!(
            "../../../crates/services/paintcraft-formats/tests/png-retention-fixtures/rgb8.png"
        );
        let legacy = image::decode_with_pixel_limit(bytes, PIXELS_MAX).unwrap();
        let budget = SharedBudget::new(METADATA_BYTES);
        with_budget(bytes, &legacy, &budget, &|| Ok(())).unwrap();
        assert_eq!(budget.usage().reserved, 0);
        let small = SharedBudget::new(1);
        assert_eq!(
            with_budget(bytes, &legacy, &small, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(small.usage().reserved, 0);
        assert_eq!(
            with_budget(bytes, &legacy, &budget, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "fuzz callback"
            )))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
