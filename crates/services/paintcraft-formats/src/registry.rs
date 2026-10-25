//! Frontend policy and qualified dispatch candidates, independent of codec enums.
//! Admission never validates a file: the bounded selected decoder must still run.
use paintcraft_core::types::{Error, ErrorCode, Result};

mod families;
pub use families::Family;
mod policy;
pub use policy::STUDIO_STOCK;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frontend {
    ClassicV7,
    ClassicV10,
    ClassicV11,
    Studio,
}
impl Frontend {
    pub fn id(self) -> &'static str {
        match self {
            Self::ClassicV7 => "classic-v7",
            Self::ClassicV10 => "classic-v10",
            Self::ClassicV11 => "classic-v11",
            Self::Studio => "studio",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Native,
    Web,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Open,
    OpenProject,
    OpenRaster,
    ImportConvert,
    ImportAsset,
    SavePreserving,
    SaveAsCopy,
    ExportFlat,
    ExportAsset,
}
impl Operation {
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::SavePreserving | Self::SaveAsCopy | Self::ExportFlat | Self::ExportAsset
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    UnsupportedFrontendFormat,
    BackendUnavailable,
    VariantUnqualified,
    ConvertRequiresConsent,
    PreservationNotPossible,
    InvalidSignature,
    ExtensionMismatch,
    UnsupportedOperation,
}
impl Reason {
    pub fn id(self) -> &'static str {
        match self {
            Self::UnsupportedFrontendFormat => "unsupported_frontend_format",
            Self::BackendUnavailable => "backend_unavailable",
            Self::VariantUnqualified => "variant_unqualified",
            Self::ConvertRequiresConsent => "convert_requires_consent",
            Self::PreservationNotPossible => "preservation_not_possible",
            Self::InvalidSignature => "invalid_signature",
            Self::ExtensionMismatch => "extension_mismatch",
            Self::UnsupportedOperation => "unsupported_operation",
        }
    }
    pub fn error(self, family: Option<Family>, frontend: Frontend, operation: Operation) -> Error {
        let code = match self {
            Self::InvalidSignature => ErrorCode::CodecFailed,
            Self::ExtensionMismatch => ErrorCode::InputSchema,
            _ => ErrorCode::CapabilityUnsupported,
        };
        Error::new(
            code,
            format!(
                "{}: format={}, frontend={frontend:?}, operation={operation:?}",
                self.id(),
                family.map_or("unknown", Family::id)
            ),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    AllowedWithPreflight(Reason),
    RequiresExplicitImport,
    TemporarilyUnavailable(Reason),
    Denied(Reason),
}
impl Decision {
    /// Discovery may show only these entries, with preflight performed at execution.
    pub fn available(self) -> bool {
        matches!(self, Self::Allowed | Self::AllowedWithPreflight(_))
    }
    /// Fail closed if a caller has not resolved required preflight/explicit import.
    pub fn require_allowed(self, request: Request<'_>) -> Result<()> {
        match self {
            Self::Allowed => Ok(()),
            Self::AllowedWithPreflight(reason)
            | Self::TemporarilyUnavailable(reason)
            | Self::Denied(reason) => {
                Err(reason.error(Some(request.family), request.frontend, request.operation))
            }
            Self::RequiresExplicitImport => Err(Reason::ConvertRequiresConsent.error(
                Some(request.family),
                request.frontend,
                request.operation,
            )),
        }
    }
}

/// Platform availability may narrow support; it cannot qualify an untested codec.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    pub frontend: Frontend,
    pub operation: Operation,
    pub family: Family,
    pub platform: Platform,
    pub extension: Option<&'a str>,
    /// None selects the adapter's documented baseline; named variants need qualification.
    pub variant: Option<&'a str>,
    pub backend_available: bool,
    /// Semantic loss identified by the document/codec preflight, never assumed consent.
    pub needs_preflight: bool,
}
impl Request<'_> {
    pub fn new(
        frontend: Frontend,
        operation: Operation,
        family: Family,
        platform: Platform,
    ) -> Self {
        Self {
            frontend,
            operation,
            family,
            platform,
            extension: None,
            variant: None,
            backend_available: true,
            needs_preflight: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capability {
    pub backend_available: bool,
    /// Qualified only for the bounded RGBA8 adapter subset, not every file variant.
    pub qualified: bool,
    pub backend: Option<&'static str>,
}
pub fn capability(family: Family, operation: Operation, _platform: Platform) -> Capability {
    let raster_operation = matches!(
        operation,
        Operation::Open
            | Operation::OpenRaster
            | Operation::ImportConvert
            | Operation::SavePreserving
            | Operation::SaveAsCopy
            | Operation::ExportFlat
    );
    let project_operation = matches!(
        operation,
        Operation::Open
            | Operation::OpenProject
            | Operation::ImportConvert
            | Operation::SavePreserving
            | Operation::SaveAsCopy
    );
    let backend = match family {
        Family::Tiff if raster_operation && operation.is_write() => Some("tiff-rgba8"),
        Family::Pcnd if project_operation => Some("paintcraft-pcnd-v1"),
        Family::Png
        | Family::Jpeg
        | Family::Bmp
        | Family::Gif
        | Family::Tiff
        | Family::Tga
        | Family::Webp
            if raster_operation =>
        {
            Some("image-rgba8")
        }
        Family::Pdn
            if matches!(
                operation,
                Operation::Open | Operation::OpenProject | Operation::ImportConvert
            ) =>
        {
            Some("paintcraft-pdn-pdn3-rgba8")
        }
        Family::Paint if operation == Operation::ImportConvert => {
            Some("paintcraft-paint-iovl-unci")
        }
        Family::Ora if matches!(operation, Operation::ImportConvert | Operation::SaveAsCopy) => {
            Some("openraster-flat-png")
        }
        Family::Dcx if operation == Operation::ImportConvert => Some("pcx-dcx-page"),
        Family::Qoi | Family::Pnm | Family::Farbfeld
            if matches!(operation, Operation::ImportConvert | Operation::ExportFlat) =>
        {
            Some("image-extended-rgba8")
        }
        Family::Pcx if matches!(operation, Operation::ImportConvert | Operation::ExportFlat) => {
            Some("pcx-rgba8")
        }
        Family::Jxl if raster_operation && operation.is_write() => Some("zune-jpegxl"),
        Family::Jxl if raster_operation => Some("jxl-rs"),
        Family::Dds if raster_operation => Some("paintcraft-dds-rgba8"),
        Family::Avif if raster_operation && operation.is_write() => Some("image-ravif"),
        // Recognized families do not acquire operations without a decoder.
        _ => None,
    };
    let qualified = backend.is_some()
        && match family {
            Family::Pcnd => project_operation,
            Family::Png | Family::Jpeg | Family::Bmp => raster_operation,
            Family::Gif | Family::Tiff | Family::Tga | Family::Webp | Family::Dds => matches!(
                operation,
                Operation::Open
                    | Operation::OpenRaster
                    | Operation::ImportConvert
                    | Operation::ExportFlat
            ),
            Family::Qoi | Family::Pnm | Family::Farbfeld | Family::Pcx => {
                matches!(operation, Operation::ImportConvert | Operation::ExportFlat)
            }
            Family::Paint | Family::Dcx | Family::Ora => operation == Operation::ImportConvert,
            Family::Jxl | Family::Avif => operation == Operation::ExportFlat,
            Family::Pdn => matches!(
                operation,
                Operation::Open | Operation::OpenProject | Operation::ImportConvert
            ),
            _ => false,
        };
    Capability {
        backend_available: backend.is_some(),
        qualified,
        backend,
    }
}

pub fn admission(request: Request<'_>) -> Decision {
    if let Some(reason) = policy::deny(request) {
        return reason;
    }
    if let Some(extension) = request.extension {
        if Family::from_extension(extension) != Some(request.family) {
            return Decision::Denied(Reason::ExtensionMismatch);
        }
        if !request.family.qualified_extension(extension) {
            return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
        }
    }
    let selected_page = request.family == Family::Dcx
        && request.operation == Operation::ImportConvert
        && request.variant == Some("dcx-selected-page");
    if request.family == Family::Dcx
        && request.operation == Operation::ImportConvert
        && !selected_page
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if request.variant.is_some() && !selected_page {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if request.family == Family::Pnm
        && request.operation.is_write()
        && request
            .extension
            .is_some_and(|ext| !ext.trim_start_matches('.').eq_ignore_ascii_case("pam"))
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    let capability = capability(request.family, request.operation, request.platform);
    if !capability.backend_available || !request.backend_available {
        return Decision::TemporarilyUnavailable(Reason::BackendUnavailable);
    }
    if !capability.qualified {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if request.needs_preflight || (request.operation.is_write() && request.family != Family::Pcnd) {
        return Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent);
    }
    Decision::Allowed
}

/// Qualified discovery inventory. Project actions remain distinguishable by Family.
pub fn enabled(frontend: Frontend, operation: Operation, platform: Platform) -> Vec<Family> {
    Family::ALL
        .iter()
        .copied()
        .filter(|family| {
            admission(Request::new(frontend, operation, *family, platform)).available()
        })
        .collect()
}

/// Picker aliases are qualified individually, not copied from the planning inventory.
pub fn enabled_extensions(
    frontend: Frontend,
    operation: Operation,
    platform: Platform,
) -> Vec<&'static str> {
    enabled(frontend, operation, platform)
        .into_iter()
        .flat_map(|family| {
            family
                .extensions()
                .iter()
                .copied()
                .filter(move |extension| {
                    let mut request = Request::new(frontend, operation, family, platform);
                    request.extension = Some(extension);
                    admission(request).available()
                })
        })
        .collect()
}

/// Resolve a dispatch candidate before decoding. The binary header identifies
/// PCND; the strict reader still validates its version, integrity and payloads.
pub fn admit_open(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
) -> Result<Family> {
    admit_input(frontend, filename, bytes, platform, Operation::Open, None)
}

/// Explicit conversion import has its own frontend policy and picker inventory.
/// It still validates signatures/suffixes and never enables a preserving save.
pub fn admit_import(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
) -> Result<Family> {
    admit_input(
        frontend,
        filename,
        bytes,
        platform,
        Operation::ImportConvert,
        None,
    )
}

/// DCX import is available only with an explicit selected page. It is excluded
/// from ordinary pickers until a page selector exists in that frontend.
pub fn admit_import_page(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    page: u16,
) -> Result<Family> {
    let family = admit_input(
        frontend,
        filename,
        bytes,
        platform,
        Operation::ImportConvert,
        Some("dcx-selected-page"),
    )?;
    if family != Family::Dcx || crate::dcx::inspect(bytes)?.get(usize::from(page)).is_none() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "DCX page index is out of range",
        ));
    }
    Ok(family)
}

fn admit_input(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    operation: Operation,
    variant: Option<&str>,
) -> Result<Family> {
    let extension = filename
        .rsplit(['/', '\\'])
        .next()
        .and_then(|name| name.rsplit_once('.').map(|(_, ext)| ext));
    let hint = extension.and_then(Family::from_extension);
    if let Some(family) = hint {
        let request = Request::new(frontend, operation, family, platform);
        // Refuse foreign projects/vectors before content acquisition or codec fallback.
        if let Some(decision) = policy::deny(request) {
            decision.require_allowed(request)?;
        }
    }
    // MIF1 is shared with HEIF. A .paint suffix chooses a conversion candidate,
    // never proves layer semantics; the strict item/pixel decoder still must run.
    let paint_candidate = operation == Operation::ImportConvert
        && hint == Some(Family::Paint)
        && bytes.len() >= 16
        && bytes.get(4..12) == Some(b"ftypmif1");
    let sniffed = if paint_candidate {
        Some(Family::Paint)
    } else {
        sniff(bytes)
    }
    .ok_or_else(|| Reason::InvalidSignature.error(hint, frontend, operation))?;
    let mut request = Request::new(frontend, operation, sniffed, platform);
    request.extension = extension;
    request.variant = variant;
    admission(request).require_allowed(request)?;
    Ok(sniffed)
}

/// Recognize signatures without invoking a decoder or trusting an extension.
pub fn sniff(bytes: &[u8]) -> Option<Family> {
    if crate::ora::has_signature(bytes) {
        Some(Family::Ora)
    } else if bytes.len() >= 8 && bytes.starts_with(b"PCND") && bytes[5..8] == [13, 10, 26] {
        Some(Family::Pcnd)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Family::Png)
    } else if bytes.starts_with(&[255, 216, 255]) {
        Some(Family::Jpeg)
    } else if bytes.starts_with(b"BM") {
        Some(Family::Bmp)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Family::Gif)
    } else if bytes.starts_with(b"II\x2a\0") || bytes.starts_with(b"MM\0\x2a") {
        Some(Family::Tiff)
    } else if bytes.starts_with(b"PDN3") {
        Some(Family::Pdn)
    } else if bytes.starts_with(b"8BPS\0\x01") {
        Some(Family::Psd)
    } else if bytes.starts_with(b"8BPS\0\x02") {
        Some(Family::Psb)
    } else if bytes.starts_with(b"DDS ") {
        Some(Family::Dds)
    } else if bytes.starts_with(&987_654_321u32.to_le_bytes()) {
        Some(Family::Dcx)
    } else if bytes.starts_with(b"qoif") {
        Some(Family::Qoi)
    } else if bytes.starts_with(b"farbfeld") {
        Some(Family::Farbfeld)
    } else if matches!(bytes.get(..2), Some([b'P', b'1'..=b'7']))
        && bytes.get(2).is_some_and(u8::is_ascii_whitespace)
    {
        Some(Family::Pnm)
    } else if bytes.len() >= 128
        && bytes[0] == 0x0a
        && bytes[2] == 1
        && matches!(bytes[3], 1 | 2 | 4 | 8)
    {
        Some(Family::Pcx)
    } else if bytes.starts_with(b"%PDF-") {
        Some(Family::Pdf)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(Family::Webp)
    } else if crate::image::ImageFormat::sniff(bytes) == Some(crate::image::ImageFormat::Tga) {
        Some(Family::Tga)
    } else {
        None
    }
}
