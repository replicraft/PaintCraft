//! Explicit Studio ORA copy uses the existing layered reviewer and frozen source.
use super::*;
use paintcraft_formats::{
    ora,
    registry::{self, Platform},
};
impl App {
    pub(crate) fn try_import_ora_source(&mut self, path: &Path, convert: bool) -> Result<bool> {
        if !convert
            || path
                .extension()
                .is_some_and(|ext| !ext.eq_ignore_ascii_case("ora"))
        {
            return Ok(false);
        }
        let source = if path.extension().is_none() {
            let Some(source) = crate::format_admission::acquire_if_ora(path)? else {
                return Ok(false);
            };
            source
        } else {
            crate::format_admission::acquire_resource_source(
                path,
                ora::CONVERSION_BYTES_MAX as u64,
            )?
        };
        registry::admit_import_ora(
            self.format_frontend(),
            &path.to_string_lossy(),
            &source.bytes,
            Platform::Native,
        )?;
        self.guard_asset_adoption(path, Some(&source.identity))?;
        let sha256: [u8; 32] = Sha256::digest(&source.bytes).into();
        let converted = paintcraft_formats::project_import::decode_ora_with_neutral_groups(
            &source.bytes,
            4_194_304,
            &|| Ok(()),
        )?;
        source
            .identity
            .verify_bytes(path, source.bytes.len() as u64, &sha256, &|| Ok(()))?;
        let guard = crate::destination::SourceGuard {
            path: path.to_path_buf(),
            identity: source.identity.clone(),
        };
        self.stage_project_import(converted, path.to_path_buf())?;
        let pending = self.pending_import.as_mut().unwrap();
        pending.captured_source = Some(CapturedSource {
            identity: source.identity,
            bytes: source.bytes.len() as u64,
            sha256,
        });
        pending.auxiliary_sources.push(guard);
        Ok(true)
    }
}

#[cfg(test)]
#[path = "ora/tests.rs"]
mod tests;
