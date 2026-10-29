//! File association and synchronous saves; asynchronous coordination lives in saving.
use super::*;
use paintcraft_core::types::Error;

impl Editor {
    /// Prepare editable text privately for an export preview without publication.
    pub(crate) fn export_snapshot(&self) -> Result<Arc<Document>> {
        if !self.can_capture_file() || self.has_floating_selection() || self.is_constructing_shape()
        {
            return Err(Error::new(
                paintcraft_core::types::ErrorCode::InputSchema,
                "Finish pending edits before preparing an export preview",
            ));
        }
        let Some(action) = self.tools.pending_text_action() else {
            return Ok(self.session.snapshot());
        };
        let preview = background::preview(&action);
        let Some(batch) = self.prepare_preview(preview).build(|| Ok(()))? else {
            return Ok(self.session.snapshot());
        };
        Ok(self
            .session
            .preparation()?
            .apply(&batch, || Ok(()))?
            .document_snapshot())
    }
    pub fn open(path: &Path) -> Result<Editor> {
        let (bytes, binding) = crate::destination::acquire(path)?;
        // Standalone controllers share the same process-owned metadata realm as
        // App/service controllers; this clone is not a fresh per-read allowance.
        let metadata = paintcraft_raster::Admission::new(0).paint_metadata_budget();
        let document = pcnd::read_with_metadata_admission_checked(
            &bytes,
            u64::MAX,
            paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            paintcraft_document::LAYER_COUNT_MAX,
            pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: &metadata,
            },
            &|| Ok(()),
        )?;
        let mut e = Editor::with_document(document, Some(path.to_path_buf()));
        e.destination = Some(binding);
        e.status = format!("Opened {}", path.display());
        Ok(e)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: PathBuf) {
        if self.path.as_ref() != Some(&path) {
            self.destination = Some(crate::destination::Binding::authorized());
        }
        self.path = Some(path);
    }

    pub(crate) fn destination_binding(&self) -> Option<crate::destination::Binding> {
        self.destination.clone()
    }
    pub(crate) fn bind_destination(&mut self, binding: Option<crate::destination::Binding>) {
        self.destination = binding;
    }
    pub(crate) fn file_association(&self) -> crate::destination::Previous {
        crate::destination::Previous {
            path: self.path.clone(),
            binding: self.destination.clone(),
        }
    }
    pub(crate) fn restore_path(&mut self, previous: crate::destination::Previous) {
        self.path = previous.path;
        self.destination = previous.binding;
    }

    pub(super) fn save(&mut self) {
        self.save_outcome = Some(false);
        let Some(path) = self.path.clone() else {
            self.status = "Choose a file name before saving".into();
            return;
        };
        let Some(binding) = self.destination.clone() else {
            self.status = "Not saved safely: choose Save As to authorize a destination".into();
            return;
        };
        let outcome = binding.save(|expected| {
            save::save_with_expectation_and_preparation(
                &path,
                true,
                save::STAGED_FILE_BYTES_MAX,
                expected,
                |sink| pcnd::write_compact_to(self.document(), sink),
                |_| Ok(()),
            )
        });
        if outcome.is_success() {
            self.save_outcome = Some(true);
            self.session.mark_saved();
            self.status = match self.discard_checkpoint() {
                Ok(()) => format!("Saved {}", path.display()),
                Err(error) => format!("Saved {}; recovery cleanup failed: {error}", path.display()),
            };
        } else {
            // Dirty state stays: an unconfirmed or unknown save is not a clean document.
            self.status = format!("Not saved safely: {}", outcome.reasons.join("; "));
        }
    }
}

#[cfg(test)]
mod export_preview_tests {
    use super::*;
    #[test]
    fn export_snapshot_keeps_text_private_and_matches_actual_placement() {
        let mut editor = Editor::new_document(160, 64).unwrap();
        editor.select_tool(Tool::Text);
        editor.pointer_down((4, 4));
        editor.pointer_up();
        editor.type_text("Ink");
        assert!(editor.is_editing_text());
        let original = editor.document_snapshot();
        let ticket = editor.history_view().ticket;
        let copy = editor.export_snapshot().unwrap();
        assert!(!copy.same_semantics(&original));
        assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
        assert_eq!(editor.history_view().ticket, ticket);
        assert!(editor.is_editing_text());
        let preview = paintcraft_render::render_flat(&copy).unwrap();
        editor.commit_text();
        editor.run_pending();
        assert_eq!(
            paintcraft_render::render_flat(editor.document()).unwrap(),
            preview
        );
    }
}
