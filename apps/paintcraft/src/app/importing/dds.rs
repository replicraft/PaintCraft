//! DDS keeps the existing raster loss reviewer and exact source descriptor.
use super::*;
use paintcraft_formats::registry::{self, Family, Platform};
impl App {
    pub(crate) fn try_open_dds_source(&mut self, path: &Path, convert: bool) -> Result<bool> {
        if path
            .extension()
            .is_some_and(|ext| !ext.eq_ignore_ascii_case("dds"))
        {
            return Ok(false);
        }
        let source = if path.extension().is_none() {
            let Some(source) = crate::format_admission::acquire_if_dds(path)? else {
                return Ok(false);
            };
            source
        } else {
            crate::format_admission::acquire_resource_source(
                path,
                paintcraft_formats::dds::BYTES_MAX as u64,
            )?
        };
        let family = if convert {
            registry::admit_import(
                self.format_frontend(),
                &path.to_string_lossy(),
                &source.bytes,
                Platform::Native,
            )?
        } else {
            self.admit_open_source(path, &source.bytes)?
        };
        if family != Family::Dds {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "DDS filename and source differ",
            ));
        }
        self.guard_asset_adoption(path, Some(&source.identity))?;
        let sha256: [u8; 32] = Sha256::digest(&source.bytes).into();
        let image = image::decode(&source.bytes)?;
        source
            .identity
            .verify_bytes(path, source.bytes.len() as u64, &sha256, &|| Ok(()))?;
        let guard = crate::destination::SourceGuard {
            path: path.to_path_buf(),
            identity: source.identity.clone(),
        };
        if !image.losses.is_empty() {
            self.stage_import(image, path.to_path_buf(), hex(&sha256), convert)?;
            let pending = self.pending_import.as_mut().unwrap();
            pending.captured_source = Some(CapturedSource {
                identity: source.identity,
                bytes: source.bytes.len() as u64,
                sha256,
            });
            pending.auxiliary_sources.push(guard);
        } else {
            let doc = import_image(image.width, image.height, &image.rgba, &hex(&sha256))?;
            let mut editor = Editor::with_document(doc, None);
            if convert {
                editor.invalidate_saved_state();
            }
            self.install(editor)?;
            self.bound = None;
            self.imported_source = Some(path.to_path_buf());
            self.auxiliary_sources = vec![guard];
            self.path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
            self.editor
                .set_status(format!("Imported {}", path.display()));
            if !convert {
                self.recent.remember(path);
            }
        }
        Ok(true)
    }
}
#[cfg(test)]
#[path = "dds/tests.rs"]
mod tests;
