//! One frontend policy for launch, Open, Recent, drop and shell paths.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::registry::{Family, Frontend, Platform};
use paintcraft_ui::t;
use std::io::Read;

const SOURCE_BYTES: u64 = 512 << 20;

impl App {
    pub(super) fn guard_asset_adoption(
        &self,
        path: &Path,
        identity: Option<&crate::destination::SourceIdentity>,
    ) -> Result<()> {
        if self
            .saving
            .as_ref()
            .map(|saving| saving.has_pending_write_to(path, identity))
            .transpose()?
            .unwrap_or(false)
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                t!(
                    &self.context,
                    "The resource source has a pending project save. Wait for it to finish and import again."
                ),
            ));
        }
        Ok(())
    }
    fn source_destination_matches(&self, path: &Path, documents: bool) -> Result<bool> {
        let document_sources = if documents {
            self.document_source_paths()
        } else {
            Vec::new()
        };
        if self
            .asset_source_paths()
            .chain(document_sources.iter().copied())
            .any(|source| crate::tabs::same_destination(source, path))
        {
            return Ok(true);
        }
        let mut identities = self
            .palettes
            .source_identity()
            .into_iter()
            .chain(self.shapes.source_identity())
            .chain(self.shapes.pending_source_identity())
            .chain(self.brushes.source_identity())
            .chain(
                self.auxiliary_source_guards()
                    .into_iter()
                    .map(|source| &source.identity),
            )
            .peekable();
        if identities.peek().is_none() && document_sources.is_empty() {
            return Ok(false);
        }
        // Capture the destination once for all loaded and parked owners.
        let Some(candidate) = crate::destination::SourceIdentity::existing_destination(path)?
        else {
            return Ok(false);
        };
        if identities.any(|identity| identity.matches(&candidate)) {
            return Ok(true);
        }
        // Ordinary painting associations retain their existing current-path
        // comparison. Only acquired SVG/resource inputs have frozen identities.
        Ok(document_sources.into_iter().any(|source| {
            crate::destination::SourceIdentity::existing_destination(source)
                .ok()
                .flatten()
                .is_some_and(|identity| identity.matches(&candidate))
        }))
    }
    pub(super) fn asset_source_paths(&self) -> impl Iterator<Item = &Path> {
        self.palettes
            .source_path()
            .into_iter()
            .chain(self.shapes.source_path())
            .chain(self.shapes.pending_source_path())
            .chain(self.brushes.source_path())
            .chain(
                self.auxiliary_source_guards()
                    .into_iter()
                    .map(|source| source.path.as_path()),
            )
    }
    pub(super) fn guard_save_destination(&self, path: &Path) -> Result<()> {
        if self.source_destination_matches(path, false)? {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "a project save must not replace a loaded resource source",
            ));
        }
        Ok(())
    }
    pub(super) fn guard_export_destination(&self, path: &Path) -> Result<()> {
        if self.source_destination_matches(path, true)? {
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

pub(super) fn read_source(path: &Path) -> Result<Vec<u8>> {
    read_source_bounded(path, SOURCE_BYTES)
}
pub(super) fn read_source_bounded(path: &Path, allowance: u64) -> Result<Vec<u8>> {
    let allowance = allowance.min(SOURCE_BYTES);
    read_limited(open_source(path, allowance)?, allowance)
}

#[derive(Debug)]
pub(super) struct ResourceSource {
    pub(super) bytes: Vec<u8>,
    pub(super) identity: crate::destination::SourceIdentity,
}
pub(super) fn acquire_resource_source(path: &Path, allowance: u64) -> Result<ResourceSource> {
    let allowance = allowance.min(SOURCE_BYTES);
    let identity = crate::destination::SourceIdentity::from_file(open_source(path, allowance)?)?;
    // Read through the exact retained handle. Never reopen a path to attach an
    // identity after decoding bytes acquired from a different file.
    let bytes = read_limited(identity.file(), allowance)?;
    Ok(ResourceSource { bytes, identity })
}
/// Probe only the signature under the generic allowance; a selected DDS keeps this descriptor.
pub(super) fn acquire_if_dds(path: &Path) -> Result<Option<ResourceSource>> {
    acquire_if_signature(path, b"DDS ", paintcraft_formats::dds::BYTES_MAX as u64)
}
pub(super) fn acquire_if_ora(path: &Path) -> Result<Option<ResourceSource>> {
    acquire_if_signature(
        path,
        b"PK\x03\x04",
        paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64,
    )
}
fn acquire_if_signature(
    path: &Path,
    wanted: &[u8; 4],
    allowance: u64,
) -> Result<Option<ResourceSource>> {
    use std::io::{Seek, SeekFrom};
    let mut file = open_source(path, SOURCE_BYTES)?;
    let mut signature = [0; 4];
    match file.read_exact(&mut signature) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(storage(error)),
    }
    if &signature != wanted {
        return Ok(None);
    }
    if file.metadata().map_err(storage)?.len() > allowance {
        return Err(limit());
    }
    file.seek(SeekFrom::Start(0)).map_err(storage)?;
    let identity = crate::destination::SourceIdentity::from_file(file)?;
    let bytes = read_limited(identity.file(), allowance)?;
    Ok(Some(ResourceSource { bytes, identity }))
}
fn open_source(path: &Path, allowance: u64) -> Result<std::fs::File> {
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
    Ok(file)
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
