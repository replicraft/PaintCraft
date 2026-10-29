//! Resident document identity includes the full selected-page conversion.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
pub(super) fn assemble(
    report: &tiff_pages::Assessment,
    rgba: &[u8],
    residency: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    report.validate_result(
        report.source_sha256(),
        report.source_bytes(),
        report.page_index(),
        report.width(),
        report.height(),
        rgba,
    )?;
    let mut admission = paintcraft_raster::Admission::with_residency(
        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
        residency,
    );
    paintcraft_document::import_image_with_admission_checked(
        report.width(),
        report.height(),
        rgba,
        &paintcraft_document::hex(&report.document_identity()),
        &mut admission,
        check,
    )
}
