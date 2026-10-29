//! Optional frontend context uses the same pure policy as desktop and browser.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum Profile {
    ClassicV7,
    ClassicV10,
    ClassicV11,
    #[default]
    Studio,
}
impl From<Profile> for Frontend {
    fn from(value: Profile) -> Self {
        match value {
            Profile::ClassicV7 => Self::ClassicV7,
            Profile::ClassicV10 => Self::ClassicV10,
            Profile::ClassicV11 => Self::ClassicV11,
            Profile::Studio => Self::Studio,
        }
    }
}
#[derive(clap::Args, Debug, Default)]
#[group(id = "frontend-profile")]
pub struct Options {
    /// Restrict operations to this frontend's qualified format profile.
    #[arg(long, value_enum, default_value = "studio")]
    pub frontend: Profile,
}
impl Options {
    pub fn admit_open(&self, path: &std::path::Path, bytes: &[u8]) -> Result<Family> {
        registry::admit_open(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
        )
    }
    pub fn admit_import(&self, path: &std::path::Path, bytes: &[u8]) -> Result<Family> {
        registry::admit_import(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
        )
    }
    pub fn admit_import_ora(&self, path: &std::path::Path, bytes: &[u8]) -> Result<Family> {
        registry::admit_import_ora(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
        )
    }
    pub fn admit_import_svg(&self, path: &std::path::Path, bytes: &[u8]) -> Result<Family> {
        registry::admit_import_svg(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
        )
    }
    /// Refuse incompatible profiles and filenames before acquiring SVG content.
    pub fn admit_import_svg_path(&self, path: &std::path::Path) -> Result<()> {
        let mut request = Request::new(
            self.frontend.into(),
            Operation::ImportConvert,
            Family::Svg,
            Platform::Native,
        );
        request.extension = path.extension().and_then(|value| value.to_str());
        request.variant = Some(paintcraft_formats::svg::IMPORT_VARIANT);
        let decision = registry::admission(request);
        if decision.available() {
            Ok(())
        } else {
            decision.require_allowed(request)
        }
    }
    pub fn admit_import_native_path(&self, path: &std::path::Path) -> Result<()> {
        let family = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(Family::from_extension)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InputSchema,
                    "Native codec filename is not qualified",
                )
            })?;
        let variant = match family {
            Family::Jxl => paintcraft_formats::codec_worker::NATIVE_JXL_IMPORT_VARIANT,
            Family::Jxr => paintcraft_formats::codec_worker::NATIVE_JXR_IMPORT_VARIANT,
            Family::Heif => paintcraft_formats::codec_worker::NATIVE_HEIF_IMPORT_VARIANT,
            Family::Avif => paintcraft_formats::codec_worker::NATIVE_AVIF_IMPORT_VARIANT,
            _ => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Native codec input requires JXL/JXR/HEIF/AVIF",
                ));
            }
        };
        self.admit_import_native_variant(path, family, variant)
    }
    pub fn admit_import_jxl_codec(
        &self,
        path: &std::path::Path,
        codec: paintcraft_formats::codec_worker::Codec,
    ) -> Result<()> {
        use paintcraft_formats::codec_worker::{self as wire, Codec};
        let variant = match codec {
            Codec::JxlRgba8 => wire::NATIVE_JXL_IMPORT_VARIANT,
            Codec::JxlIntegerRgba8 => wire::jxl_integer::contracts::IMPORT_VARIANT,
            _ => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Unqualified JXL selected codec",
                ));
            }
        };
        self.admit_import_native_variant(path, Family::Jxl, variant)
    }
    pub fn admit_import_jxr_assessment(
        &self,
        path: &std::path::Path,
        assessment: &paintcraft_formats::jxr::NativeImportAssessment,
    ) -> Result<()> {
        let variant = if assessment.is_writer_compatible() {
            paintcraft_formats::codec_worker::jxr_encode::qualified::WRITER_IMPORT_VARIANT
        } else {
            paintcraft_formats::codec_worker::NATIVE_JXR_IMPORT_VARIANT
        };
        self.admit_import_native_variant(path, Family::Jxr, variant)
    }
    pub fn admit_import_avif_codec(
        &self,
        path: &std::path::Path,
        codec: paintcraft_formats::codec_worker::Codec,
    ) -> Result<()> {
        let variant = match codec {
            paintcraft_formats::codec_worker::Codec::AvifSrgb8 => {
                paintcraft_formats::codec_worker::NATIVE_AVIF_IMPORT_VARIANT
            }
            paintcraft_formats::codec_worker::Codec::AvifSdr8 => {
                paintcraft_formats::codec_worker::NATIVE_AVIF_SDR_IMPORT_VARIANT
            }
            _ => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Unqualified AVIF selected codec",
                ));
            }
        };
        self.admit_import_native_variant(path, Family::Avif, variant)
    }
    pub fn admit_import_native_variant(
        &self,
        path: &std::path::Path,
        family: Family,
        variant: &str,
    ) -> Result<()> {
        let mut request = Request::new(
            self.frontend.into(),
            Operation::ImportConvert,
            family,
            Platform::Native,
        );
        request.extension = path.extension().and_then(|ext| ext.to_str());
        request.variant = Some(variant);
        match registry::admission(request) {
            registry::Decision::Allowed => Ok(()),
            registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent)
                if matches!(
                    family,
                    Family::Jxl | Family::Jxr | Family::Heif | Family::Avif
                ) =>
            {
                Ok(())
            }
            decision => decision.require_allowed(request),
        }
    }
    pub fn admit_import_page(
        &self,
        path: &std::path::Path,
        bytes: &[u8],
        page: u16,
    ) -> Result<Family> {
        registry::admit_import_page(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
            page,
        )
    }
    pub fn admit_import_entry(
        &self,
        path: &std::path::Path,
        bytes: &[u8],
        entry: u16,
    ) -> Result<Family> {
        registry::admit_import_entry(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
            entry,
        )
    }
    /// Availability gate only: export's existing loss preflight still runs next.
    pub fn admit_export(&self, family: Family, path: &std::path::Path) -> Result<()> {
        let mut request = Request::new(
            self.frontend.into(),
            Operation::ExportFlat,
            family,
            Platform::Native,
        );
        request.extension = path.extension().and_then(|value| value.to_str());
        let decision = registry::admission(request);
        if decision.available() {
            Ok(())
        } else {
            decision.require_allowed(request)
        }
    }
    pub fn reject_project_import(&self, family: Family) -> Result<()> {
        if family.is_project() {
            Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Import expects a qualified raster; use project commands for PCND",
            ))
        } else {
            Ok(())
        }
    }
}
