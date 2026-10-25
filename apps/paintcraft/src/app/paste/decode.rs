//! The output grant is acquired before this worker enters any image codec.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetReservation};
use paintcraft_platform::clipboard::Encoding;
use paintcraft_ui::i18n::import_loss_description;

pub(super) fn source(
    encoded: EncodedImage,
    frontend: Frontend,
    reservation: BudgetReservation,
    cancelled: Arc<AtomicBool>,
) -> Result<Decoded> {
    let check = || {
        if cancelled.load(Ordering::Acquire) {
            Err(Error::new(ErrorCode::OperationCancelled, "Paste cancelled"))
        } else {
            Ok(())
        }
    };
    check()?;
    let path = match encoded.encoding() {
        Encoding::Png => "clipboard.png",
        Encoding::Tiff => "clipboard.tiff",
    };
    registry::admit_open(frontend, path, encoded.bytes(), Platform::Native)?;
    let decoded = image::decode_with_pixel_limit(encoded.bytes(), MAX_PIXELS)?;
    check()?;
    if decoded
        .losses
        .iter()
        .any(|loss| import_loss_description(loss).is_none())
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Unrecognized clipboard import limitation",
        ));
    }
    let image = Image::from_rgba_reserved(
        decoded.width,
        decoded.height,
        decoded.rgba,
        &reservation,
        &check,
    )?;
    // Owned function arguments drop before the worker sends the result: the
    // encoded snapshot and unused output grant are retired before UI readiness.
    Ok(Decoded {
        image,
        losses: decoded.losses,
    })
}
