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
        let (image, raster_key, source_metadata) = if paintcraft_formats::registry::sniff(&bytes)
            == Some(paintcraft_formats::registry::Family::Png)
        {
            let budget = self.source_metadata_budget();
            let prepared = image::png_retention::decode(
                &bytes,
                image::DECODE_PIXELS_MAX,
                paintcraft_formats::pcnd::MetadataAdmission {
                    bytes: paintcraft_document::source_metadata::BYTES_MAX,
                    budget: &budget,
                },
                &|| Ok(()),
            )?;
            (
                prepared.image,
                Some(prepared.document_key),
                prepared.attachment,
            )
        } else {
            let image = image::decode(&bytes)?;
            let key = image::document_conversion_key(&bytes, &image, &|| Ok(()))?;
            (image, key, None)
        };
        identity.verify_bytes(&path, bytes.len() as u64, &sha256, &|| Ok(()))?;
        let guard = crate::destination::SourceGuard {
            path: path.clone(),
            identity: identity.clone(),
        };
        if !image.losses.is_empty() {
            self.stage_import(image, path, hex(&sha256), convert)?;
            let pending = self.pending_import.as_mut().unwrap();
            pending.raster_key = raster_key;
            pending.source_metadata = source_metadata;
            pending.captured_source = Some(CapturedSource {
                identity,
                bytes: bytes.len() as u64,
                sha256,
            });
            pending.auxiliary_sources.push(guard);
            return Ok(());
        }
        let key = raster_key
            .map(|key| key.hex())
            .unwrap_or_else(|| hex(&sha256));
        let doc = import_image(image.width, image.height, &image.rgba, &key)?
            .with_source_metadata_checked(source_metadata, &|| Ok(()))?;
        paintcraft_formats::pcnd::check_metadata_admission(&doc, &|| Ok(()))?;
        identity.verify_bytes(&path, bytes.len() as u64, &sha256, &|| Ok(()))?;
        let mut editor = Editor::with_document(doc, None);
        if convert {
            editor.invalidate_saved_state();
        }
        self.install(editor)?;
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
