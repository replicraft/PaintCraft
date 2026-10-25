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
    pub fn admit_import_svg(&self, path: &std::path::Path, bytes: &[u8]) -> Result<Family> {
        registry::admit_import_svg(
            self.frontend.into(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
        )
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
