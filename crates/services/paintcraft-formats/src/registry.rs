//! Frontend policy and qualified dispatch candidates, independent of codec enums.
//! Admission never replaces bounded payload decoding and loss validation.
use paintcraft_core::types::{Error, ErrorCode, Result};

mod families;
pub use families::Family;
mod input;
mod policy;
pub use input::sniff;
use input::{admit_input, admit_input_with_preflight};
mod svg;
pub use policy::STUDIO_STOCK;
pub use svg::admit_import_svg;
pub const WEB_JXL_IMPORT_VARIANT: &str = "web-jxl-srgb8-worker-quarantine-v1";
pub const WEB_JXL_INTEGER_IMPORT_VARIANT: &str = "web-jxl-integer-srgb8-worker-quarantine-v1";

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
    /// Qualified only for the bounded adapter profile, not every file variant
    /// or the separate cross-platform release gates.
    pub qualified: bool,
    pub backend: Option<&'static str>,
}
pub fn capability(family: Family, operation: Operation, platform: Platform) -> Capability {
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
        Family::Msp if matches!(operation, Operation::ImportConvert | Operation::ExportFlat) => {
            Some("paintcraft-msp-monochrome-v1")
        }
        Family::Abr if operation == Operation::ImportAsset => Some("ag-psd-abr-static-tip-v1"),
        Family::Ase if matches!(operation, Operation::ImportAsset | Operation::ExportAsset) => {
            Some("ag-psd-ase-finite-f32")
        }
        Family::Csh if matches!(operation, Operation::ImportAsset | Operation::ExportAsset) => {
            Some("ag-psd-csh-exact-i32")
        }
        Family::Tiff if raster_operation && operation.is_write() => Some("tiff-rgba8"),
        Family::Pcnd if project_operation => Some("paintcraft-pcnd-v1-v2"),
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
        Family::Psd | Family::Psb if operation == Operation::ImportConvert => {
            Some("ag-psd-rgb8-raster")
        }
        Family::Ora if matches!(operation, Operation::ImportConvert | Operation::SaveAsCopy) => {
            Some("openraster-flat-png")
        }
        Family::Dcx if operation == Operation::ImportConvert => Some("pcx-dcx-page"),
        Family::Dcx if operation == Operation::SaveAsCopy => Some("pcx-dcx-single-page-copy"),
        Family::Svg if operation == Operation::ImportConvert => Some("resvg-srgb-raster"),
        Family::Icns if operation == Operation::ImportConvert => Some("icns-selected-entry"),
        Family::Icns if operation == Operation::SaveAsCopy => Some("icns-fresh-png-entry-set"),
        Family::Ico | Family::Cur if operation == Operation::ImportConvert => {
            Some("ico-selected-entry")
        }
        Family::Ico | Family::Cur if operation == Operation::SaveAsCopy => {
            Some("ico-fresh-entry-set")
        }
        Family::Qoi | Family::Pnm | Family::Farbfeld
            if matches!(operation, Operation::ImportConvert | Operation::ExportFlat) =>
        {
            Some("image-extended-rgba8")
        }
        Family::Pcx if matches!(operation, Operation::ImportConvert | Operation::ExportFlat) => {
            Some("pcx-rgba8")
        }
        Family::Heif if operation == Operation::ImportConvert && platform == Platform::Native => {
            Some("heic-decoder-isolated-srgb8")
        }
        Family::Avif if operation == Operation::ImportConvert && platform == Platform::Native => {
            Some("dav1d-isolated-srgb8")
        }
        Family::Hdr if operation == Operation::ImportConvert && platform == Platform::Native => {
            Some("radiance-isolated-sdr8-settings32-report96")
        }
        Family::Exr if operation == Operation::ImportConvert && platform == Platform::Native => {
            Some("exr-isolated-sdr8-settings32-report96")
        }
        Family::Jxr if operation == Operation::ImportConvert && platform == Platform::Native => {
            Some("jpegxr-pure-rs-isolated-rgb8")
        }
        Family::Jxr if operation == Operation::ExportFlat && platform == Platform::Native => {
            Some("jpegxr-pure-rs-isolated-rgb8-lossless")
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
            Family::Qoi | Family::Pnm | Family::Farbfeld | Family::Pcx | Family::Msp => {
                matches!(operation, Operation::ImportConvert | Operation::ExportFlat)
            }
            Family::Ico | Family::Cur | Family::Icns => {
                matches!(operation, Operation::ImportConvert | Operation::SaveAsCopy)
            }
            Family::Paint | Family::Dcx | Family::Ora | Family::Psd | Family::Psb => {
                operation == Operation::ImportConvert
            }
            Family::Jxl | Family::Avif => operation == Operation::ExportFlat,
            Family::Svg => operation == Operation::ImportConvert,
            Family::Abr => operation == Operation::ImportAsset,
            Family::Ase | Family::Csh => {
                matches!(operation, Operation::ImportAsset | Operation::ExportAsset)
            }
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
    let native_jxl_import = request.family == Family::Jxl
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::codec_worker::NATIVE_JXL_IMPORT_VARIANT);
    // This variant is requested only after sealed helper capability selection.
    // It does not widen ordinary Open, picker inventory, or the legacy profile.
    let native_jxl_integer_import = request.family == Family::Jxl
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::codec_worker::jxl_integer::contracts::IMPORT_VARIANT);
    let native_jxr_import = request.family == Family::Jxr
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && matches!(
            request.variant,
            Some(crate::codec_worker::NATIVE_JXR_IMPORT_VARIANT)
                | Some(crate::codec_worker::jxr_encode::qualified::WRITER_IMPORT_VARIANT)
        );
    let native_jxr_export = request.family == Family::Jxr
        && request.operation == Operation::ExportFlat
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::codec_worker::jxr_encode::qualified::EXPORT_VARIANT);
    let native_dds_texture_export = request.family == Family::Dds
        && request.operation == Operation::ExportFlat
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::dds::NATIVE_TEXTURE_EXPORT_VARIANT)
        && request.extension.is_some_and(|extension| {
            extension
                .trim_start_matches('.')
                .eq_ignore_ascii_case("dds")
        });
    let web_dds_texture_export = request.family == Family::Dds
        && request.operation == Operation::ExportFlat
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Web
        && request.variant == Some(crate::dds::web_export::EXPORT_VARIANT)
        && request.extension.is_some_and(|extension| {
            extension
                .trim_start_matches('.')
                .eq_ignore_ascii_case("dds")
        });
    let web_msp2_export = request.family == Family::Msp
        && request.operation == Operation::ExportFlat
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Web
        && request.variant == Some(crate::msp::web_export::EXPORT_VARIANT)
        && request
            .extension
            .is_some_and(|value| value.trim_start_matches('.').eq_ignore_ascii_case("msp"));
    let native_msp2_export = request.family == Family::Msp
        && request.operation == Operation::ExportFlat
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::msp::NATIVE_V2_EXPORT_VARIANT)
        && request
            .extension
            .is_some_and(|value| value.trim_start_matches('.').eq_ignore_ascii_case("msp"));
    let native_heif_import = request.family == Family::Heif
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::codec_worker::NATIVE_HEIF_IMPORT_VARIANT);
    let native_avif_import = request.family == Family::Avif
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && matches!(
            request.variant,
            Some(crate::codec_worker::NATIVE_AVIF_IMPORT_VARIANT)
                | Some(crate::codec_worker::NATIVE_AVIF_SDR_IMPORT_VARIANT)
        );
    let native_float_import = request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && matches!(
            (request.family, request.variant),
            (
                Family::Hdr,
                Some(crate::codec_worker::NATIVE_HDR_SDR_IMPORT_VARIANT)
            ) | (
                Family::Exr,
                Some(crate::codec_worker::NATIVE_EXR_SDR_IMPORT_VARIANT)
            )
        );
    let web_jxl_import = request.family == Family::Jxl
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Web
        && request.variant == Some(WEB_JXL_IMPORT_VARIANT);
    let web_jxl_integer_import = request.family == Family::Jxl
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Web
        && request.variant == Some(WEB_JXL_INTEGER_IMPORT_VARIANT);
    let neutral_ora_copy = request.family == Family::Ora
        && request.operation == Operation::ImportConvert
        && request.frontend == Frontend::Studio
        && request.variant == Some(crate::ora::NEUTRAL_GROUP_IMPORT_VARIANT);
    let fresh_dcx_page = request.family == Family::Dcx
        && request.operation == Operation::SaveAsCopy
        && request.frontend == Frontend::Studio
        && request.platform == Platform::Native
        && request.variant == Some(crate::dcx::EXPORT_VARIANT);
    let selected_page = request.family == Family::Dcx
        && request.operation == Operation::ImportConvert
        && request.variant == Some("dcx-selected-page");
    let selected_tiff_page = request.family == Family::Tiff
        && request.frontend == Frontend::Studio
        && request.operation == Operation::ImportConvert
        && request.variant == Some(crate::tiff_pages::IMPORT_VARIANT);
    let selected_entry = matches!(request.family, Family::Ico | Family::Cur | Family::Icns)
        && request.operation == Operation::ImportConvert
        && request.variant == Some("icon-selected-entry");
    let fresh_entry_set = matches!(request.family, Family::Ico | Family::Cur | Family::Icns)
        && request.operation == Operation::SaveAsCopy
        && request.variant == Some("icon-fresh-entry-set");
    let svg_rasterization = request.family == Family::Svg
        && request.operation == Operation::ImportConvert
        && request.variant == Some(crate::svg::IMPORT_VARIANT);
    if request.family == Family::Svg
        && request.operation == Operation::ImportConvert
        && !svg_rasterization
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if matches!(request.family, Family::Ico | Family::Cur | Family::Icns)
        && request.operation == Operation::SaveAsCopy
        && !fresh_entry_set
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if matches!(request.family, Family::Ico | Family::Cur | Family::Icns)
        && request.operation == Operation::ImportConvert
        && !selected_entry
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if request.family == Family::Dcx
        && request.operation == Operation::ImportConvert
        && !selected_page
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if request.variant.is_some()
        && !selected_page
        && !selected_tiff_page
        && !fresh_dcx_page
        && !selected_entry
        && !fresh_entry_set
        && !svg_rasterization
        && !neutral_ora_copy
        && !native_jxl_import
        && !native_jxl_integer_import
        && !web_jxl_import
        && !web_jxl_integer_import
        && !native_jxr_import
        && !native_jxr_export
        && !web_msp2_export
        && !native_msp2_export
        && !native_dds_texture_export
        && !web_dds_texture_export
        && !native_heif_import
        && !native_avif_import
        && !native_float_import
    {
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
    if !capability.qualified
        && !fresh_dcx_page
        && !native_jxl_import
        && !native_jxl_integer_import
        && !web_jxl_import
        && !web_jxl_integer_import
        && !native_jxr_import
        && !native_jxr_export
        && !native_heif_import
        && !native_avif_import
        && !native_float_import
    {
        return Decision::TemporarilyUnavailable(Reason::VariantUnqualified);
    }
    if request.needs_preflight
        || selected_tiff_page
        || web_msp2_export
        || native_msp2_export
        || native_dds_texture_export
        || web_dds_texture_export
        || native_jxr_import
        || native_jxl_integer_import
        || web_jxl_integer_import
        || native_heif_import
        || native_avif_import
        || native_float_import
        || svg_rasterization
        || neutral_ora_copy
        || (request.operation.is_write() && request.family != Family::Pcnd)
    {
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

/// Explicit converted ORA copy; ordinary Open and baseline decoder dispatch stay separate.
pub fn admit_import_ora(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
) -> Result<Family> {
    // Baseline signature/suffix policy still applies; availability below requires
    // the caller's actual decoder preflight and loss consent before publication.
    let family = admit_import(frontend, filename, bytes, platform)?;
    if family != Family::Ora {
        return Err(Reason::InvalidSignature.error(
            Some(Family::Ora),
            frontend,
            Operation::ImportConvert,
        ));
    }
    let mut request = Request::new(frontend, Operation::ImportConvert, Family::Ora, platform);
    request.variant = Some(crate::ora::NEUTRAL_GROUP_IMPORT_VARIANT);
    let decision = admission(request);
    if !decision.available() {
        decision.require_allowed(request)?;
    }
    Ok(family)
}

/// Explicit DCX/TIFF selected-page conversion. Generic TIFF Open keeps its
/// separate strict single-page reader; this helper never performs pixel decode.
pub fn admit_import_page(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    page: u16,
) -> Result<Family> {
    if sniff(bytes) == Some(Family::Tiff) {
        let family = admit_input_with_preflight(
            frontend,
            filename,
            bytes,
            platform,
            Operation::ImportConvert,
            Some(crate::tiff_pages::IMPORT_VARIANT),
        )?;
        let inventory = crate::tiff_pages::inspect(bytes, &|| Ok(()))?;
        let selected = inventory
            .pages()
            .get(usize::from(page))
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "TIFF page index is out of range"))?;
        if let Some(reason) = selected.unsupported_reason {
            return Err(Error::new(ErrorCode::CapabilityUnsupported, reason));
        }
        return Ok(family);
    }
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

/// ICO/CUR/ICNS is an explicit asset-entry conversion, never an implicit largest
/// image. Generic import discovery excludes it until a frontend has a selector.
pub fn admit_import_entry(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    entry: u16,
) -> Result<Family> {
    let family = admit_input(
        frontend,
        filename,
        bytes,
        platform,
        Operation::ImportConvert,
        Some("icon-selected-entry"),
    )?;
    if family == Family::Icns {
        let directory = crate::icns::inspect(bytes, &|| Ok(()))?;
        let selected = directory.entries.get(usize::from(entry)).ok_or_else(|| {
            Error::new(ErrorCode::InputSchema, "ICNS entry index is out of range")
        })?;
        if selected.encoding == crate::icns::Encoding::Unqualified {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "ICNS selected entry encoding or mask pairing is unqualified",
            ));
        }
    } else if !matches!(family, Family::Ico | Family::Cur)
        || crate::icons::inspect(bytes)?
            .entries
            .get(usize::from(entry))
            .is_none()
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "icon entry index is out of range",
        ));
    }
    Ok(family)
}
