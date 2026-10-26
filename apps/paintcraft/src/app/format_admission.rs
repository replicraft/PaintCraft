//! One frontend policy for launch, Open, Recent, drop and shell paths.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::registry::{Family, Frontend, Platform};
use std::io::Read;

const SOURCE_BYTES: u64 = 512 << 20;

impl App {
    pub(super) fn asset_source_paths(&self) -> impl Iterator<Item = &Path> {
        self.palettes
            .source_path()
            .into_iter()
            .chain(self.shapes.source_path())
    }
    pub(super) fn guard_save_destination(&self, path: &Path) -> Result<()> {
        if self
            .asset_source_paths()
            .any(|source| protected_destination(source, path))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "a project save must not replace a loaded resource source",
            ));
        }
        Ok(())
    }
    pub(super) fn guard_export_destination(&self, path: &Path) -> Result<()> {
        if self
            .protected_source_paths()
            .into_iter()
            .any(|source| protected_destination(source, path))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "an exported file must not replace an open document or loaded resource source",
            ));
        }
        Ok(())
    }
    pub(super) fn format_frontend(&self) -> Frontend {
        match (self.experience(), self.variant()) {
            (Experience::Studio, _) => Frontend::Studio,
            (_, ClassicVariant::V7) => Frontend::ClassicV7,
            (_, ClassicVariant::V10) => Frontend::ClassicV10,
            (_, ClassicVariant::V11) => Frontend::ClassicV11,
        }
    }
    pub(super) fn admit_open_source(&self, path: &Path, bytes: &[u8]) -> Result<Family> {
        paintcraft_formats::registry::admit_open(
            self.format_frontend(),
            &path.to_string_lossy(),
            bytes,
            Platform::Native,
        )
    }

    pub(super) fn admit_export_format(&self, path: &Path, format: ImageFormat) -> Result<()> {
        self.guard_export_destination(path)?;
        use paintcraft_formats::registry::{self, Operation, Request};
        let family = Family::from_id(format.name()).ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Unregistered export format",
            )
        })?;
        let mut request = Request::new(
            self.format_frontend(),
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

    /// Desktop launch must load the stored profile before admitting its launch file.
    pub fn new_with_settings(path: Option<PathBuf>, settings: PathBuf) -> Self {
        let mut app = Self::new(None).with_settings(settings);
        if let Some(path) = path
            && let Err(error) = app.open_path(&path)
        {
            app.editor.set_status(format!("Cannot open: {error}"));
        }
        app.editor.select_tool(Tool::Brush);
        app
    }
}

fn protected_destination(source: &Path, path: &Path) -> bool {
    // Namespace comparison also protects the selected source path if it was
    // deleted. Identity comparison additionally catches regular-file hard links.
    crate::tabs::same_destination(source, path) || crate::destination::same_file(source, path)
}

pub(super) fn read_source(path: &Path) -> Result<Vec<u8>> {
    read_source_bounded(path, SOURCE_BYTES)
}
pub(super) fn read_source_bounded(path: &Path, allowance: u64) -> Result<Vec<u8>> {
    let allowance = allowance.min(SOURCE_BYTES);
    // Reject ordinary FIFOs/devices before opening; the post-open check remains
    // necessary. This does not promise race-proof traversal of a hostile filesystem.
    if !std::fs::metadata(path).map_err(storage)?.is_file() {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "Image source must be a regular file",
        ));
    }
    let file = std::fs::File::open(path).map_err(storage)?;
    let metadata = file.metadata().map_err(storage)?;
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "Image source must be a regular file",
        ));
    }
    if metadata.len() > allowance {
        return Err(limit());
    }
    read_limited(file, allowance)
}
fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut reader = reader.take(limit + 1);
    let mut bytes = Vec::new();
    let mut chunk = [0; 65536];
    loop {
        let count = reader.read(&mut chunk).map_err(storage)?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len() as u64 + count as u64 > limit {
            return Err(self::limit());
        }
        let required = bytes.len() + count;
        if required > bytes.capacity() {
            let capacity = bytes
                .capacity()
                .saturating_mul(2)
                .max(required)
                .min(limit as usize);
            bytes
                .try_reserve_exact(capacity - bytes.len())
                .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Source allocation failed"))?;
            if bytes.capacity() as u64 > limit {
                return Err(self::limit());
            }
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}
fn storage(error: std::io::Error) -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        format!("Cannot read source: {}", error.kind()),
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source exceeds the bounded input limit",
    )
}

#[cfg(test)]
#[path = "format_admission/asset_destinations.rs"]
mod asset_destinations;
#[cfg(test)]
#[path = "format_admission/tests.rs"]
mod tests;
