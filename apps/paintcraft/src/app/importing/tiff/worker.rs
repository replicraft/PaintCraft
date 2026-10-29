//! Source inventory and selected-page preparation never run on the GUI thread.
use super::*;
use paintcraft_core::types::resources::AllocationClass;
use paintcraft_ui::page_import::{Page, PageChoice};

pub(super) fn inventory(
    identity: SourceIdentity,
    path: PathBuf,
    length: u64,
    budget: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Arc<Frozen>> {
    // Retained bytes and bounded descriptor/chooser inventory are separately
    // admitted. The transient executor lease ends at poll, not at final consent.
    let mut account = budget.account(AllocationClass::PaintMetadata);
    account.resize(length.checked_add(1 << 20).ok_or_else(limit)?)?;
    let bytes = identity.capture_bytes(length, pages::BYTES_MAX as u64, check)?;
    let inventory = pages::inspect(&bytes, check)?;
    identity.verify_bytes(&path, length, &inventory.source_sha256(), check)?;
    let mut choices = Vec::new();
    choices
        .try_reserve_exact(inventory.pages().len())
        .map_err(|_| limit())?;
    choices.extend(inventory.pages().iter().map(|page| PageChoice {
        page: Page {
            index: page.index,
            width: page.width,
            height: page.height,
        },
        supported: page.unsupported_reason.is_none(),
    }));
    check()?;
    Ok(Arc::new(Frozen {
        bytes,
        inventory,
        choices,
        identity,
        _account: account,
    }))
}
pub(super) fn decode(
    frozen: Arc<Frozen>,
    path: PathBuf,
    index: u16,
    metadata: SharedBudget,
    residency: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Box<Prepared>> {
    frozen.verify(&path, check)?;
    // Format-specific full-source preflight stays on the worker. Static GUI
    // admission never grants an implicit page or bypasses profile consent.
    registry::admit_import_page(
        Frontend::Studio,
        &path.to_string_lossy(),
        &frozen.bytes,
        Platform::Native,
        index,
    )?;
    check()?;
    let decoded = pages::decode_page(&frozen.bytes, index, pages::PIXELS_MAX, check)?;
    decoded.assessment.validate_result(
        frozen.inventory.source_sha256(),
        frozen.inventory.source_bytes(),
        index,
        decoded.width,
        decoded.height,
        &decoded.rgba,
    )?;
    if decoded.losses.len() != decoded.assessment.losses().len()
        || decoded
            .assessment
            .losses()
            .iter()
            .any(|loss| !decoded.losses.iter().any(|actual| actual == loss))
        || decoded
            .losses
            .iter()
            .any(|loss| import_loss_description(loss).is_none())
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Unknown TIFF import limitation",
        ));
    }
    // Decoder pixels and staged sparse-tile storage are different budgets.
    // Skinny images may require many padded tiles despite few source pixels.
    let edge = u64::from(paintcraft_raster::TILE_EDGE);
    let padded = u64::from(decoded.width)
        .div_ceil(edge)
        .checked_mul(u64::from(decoded.height).div_ceil(edge))
        .and_then(|tiles| tiles.checked_mul(edge * edge))
        .filter(|pixels| *pixels <= paintcraft_core::types::policy::STAGED_PIXELS_MAX)
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "TIFF tile preparation allowance"))?;
    let mut admission = paintcraft_raster::Admission::with_residency(padded, residency);
    let document = paintcraft_document::import_image_with_admission_checked(
        decoded.width,
        decoded.height,
        &decoded.rgba,
        &hex(&decoded.assessment.document_identity()),
        &mut admission,
        check,
    )?;
    let (tw, th) = extent(decoded.width, decoded.height);
    let length = u64::from(tw) * u64::from(th) * 4;
    let mut account = metadata.account(AllocationClass::PaintMetadata);
    // Retained thumbnail is admitted separately from the later GUI upload copy.
    account.resize(length)?;
    let mut thumbnail = Vec::new();
    thumbnail
        .try_reserve_exact(length as usize)
        .map_err(|_| limit())?;
    thumbnail.resize(length as usize, 0);
    for y in 0..th {
        check()?;
        for x in 0..tw {
            let sx = u64::from(x) * u64::from(decoded.width) / u64::from(tw);
            let sy = u64::from(y) * u64::from(decoded.height) / u64::from(th);
            let from = ((sy * u64::from(decoded.width) + sx) * 4) as usize;
            let to = ((u64::from(y) * u64::from(tw) + u64::from(x)) * 4) as usize;
            thumbnail[to..to + 4].copy_from_slice(&decoded.rgba[from..from + 4]);
        }
    }
    frozen.verify(&path, check)?;
    check()?;
    Ok(Box::new(Prepared {
        frozen,
        document,
        assessment: decoded.assessment,
        losses: decoded.losses,
        thumbnail,
        thumbnail_size: [tw as usize, th as usize],
        preview_budget: metadata,
        _account: account,
    }))
}
fn extent(width: u32, height: u32) -> (u32, u32) {
    let side = width.max(height);
    if side <= 1024 {
        return (width, height);
    }
    (
        ((u64::from(width) * 1024 / u64::from(side)).max(1)) as u32,
        ((u64::from(height) * 1024 / u64::from(side)).max(1)) as u32,
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "TIFF retained allocation allowance",
    )
}
