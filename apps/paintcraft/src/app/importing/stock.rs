//! Ordinary raster copies retain raw source authority separately from image IDs.
use super::*;
impl App {
    pub(crate) fn open_raster_acquired(
        &mut self,
        bytes: Vec<u8>,
        identity: crate::destination::SourceIdentity,
        path: PathBuf,
        convert: bool,
    ) -> Result<()> {
        self.guard_asset_adoption(&path, Some(&identity))?;
        let sha256: [u8; 32] = Sha256::digest(&bytes).into();
        let image = image::decode(&bytes)?;
        identity.verify_bytes(&path, bytes.len() as u64, &sha256, &|| Ok(()))?;
        let guard = crate::destination::SourceGuard {
            path: path.clone(),
            identity: identity.clone(),
        };
        if !image.losses.is_empty() {
            self.stage_import(image, path, hex(&sha256), convert)?;
            let pending = self.pending_import.as_mut().unwrap();
            pending.captured_source = Some(CapturedSource {
                identity,
                bytes: bytes.len() as u64,
                sha256,
            });
            pending.auxiliary_sources.push(guard);
            return Ok(());
        }
        let doc = import_image(image.width, image.height, &image.rgba, &hex(&sha256))?;
        identity.verify_bytes(&path, bytes.len() as u64, &sha256, &|| Ok(()))?;
        self.install(Editor::with_document(doc, None))?;
        self.bound = None;
        self.imported_source = Some(path.clone());
        self.auxiliary_sources = vec![guard];
        self.path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
        self.editor
            .set_status(format!("Imported {}", path.display()));
        if !convert {
            self.recent.remember(&path);
        }
        Ok(())
    }
}
