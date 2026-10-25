use super::*;
pub const STUDIO_STOCK: &[Family] = &[
    Family::Pdn,
    Family::Png,
    Family::Jpeg,
    Family::Jxl,
    Family::Jxr,
    Family::Bmp,
    Family::Gif,
    Family::Tga,
    Family::Dds,
    Family::Tiff,
    Family::Heif,
    Family::Webp,
    Family::Avif,
];
const BASELINE: &[Family] = &[
    Family::Png,
    Family::Jpeg,
    Family::Bmp,
    Family::Gif,
    Family::Tiff,
];
const EXTENDED: &[Family] = &[
    Family::Paint,
    Family::Psd,
    Family::Psb,
    Family::Ora,
    Family::Ico,
    Family::Cur,
    Family::Qoi,
    Family::Exr,
    Family::Hdr,
    Family::Jp2,
    Family::Pcx,
    Family::Dcx,
    Family::Msp,
    Family::Svg,
    Family::Pnm,
    Family::Farbfeld,
    Family::Icns,
    Family::Raw,
    Family::Pdf,
    Family::Kra,
    Family::Xcf,
    Family::Animation,
];
pub(super) fn deny(request: Request<'_>) -> Option<Decision> {
    let Request {
        frontend,
        operation,
        family,
        ..
    } = request;
    let denied = |reason| Some(Decision::Denied(reason));
    if family == Family::Heif && operation.is_write() {
        return denied(Reason::UnsupportedOperation);
    }
    if family == Family::Svg && operation.is_write() {
        return denied(Reason::UnsupportedOperation);
    }
    if matches!(
        family,
        Family::Raw | Family::Pdf | Family::Kra | Family::Xcf
    ) && operation.is_write()
    {
        return denied(Reason::UnsupportedOperation);
    }
    if matches!(operation, Operation::ImportAsset | Operation::ExportAsset) {
        return if frontend == Frontend::Studio
            && family.is_asset()
            && !(operation == Operation::ExportAsset && family == Family::Abr)
        {
            None
        } else {
            denied(Reason::UnsupportedOperation)
        };
    }
    if family.is_asset() {
        return denied(Reason::UnsupportedOperation);
    }
    if operation == Operation::ExportFlat && family.is_project() {
        return denied(Reason::UnsupportedOperation);
    }
    if operation == Operation::OpenProject && !family.is_project() {
        return denied(Reason::UnsupportedOperation);
    }
    if operation == Operation::OpenRaster && family.is_project() {
        return denied(Reason::UnsupportedOperation);
    }
    if family == Family::Pcnd {
        return None;
    }
    if frontend == Frontend::Studio {
        if STUDIO_STOCK.contains(&family) {
            return None;
        }
        if EXTENDED.contains(&family) {
            return if operation == Operation::ImportConvert || operation.is_write() {
                None
            } else {
                Some(Decision::RequiresExplicitImport)
            };
        }
        return denied(Reason::UnsupportedFrontendFormat);
    }
    if BASELINE.contains(&family) {
        return None;
    }
    if operation == Operation::ImportConvert && matches!(family, Family::Pcx | Family::Msp) {
        return None;
    }
    if frontend == Frontend::ClassicV11
        && family == Family::Paint
        && !matches!(operation, Operation::OpenRaster | Operation::ExportFlat)
    {
        return None;
    }
    if frontend == Frontend::ClassicV11 && matches!(family, Family::Webp | Family::Heif) {
        // Optional reference/profile capability, not enabled by decoder presence.
        return Some(Decision::TemporarilyUnavailable(Reason::VariantUnqualified));
    }
    denied(Reason::UnsupportedFrontendFormat)
}
