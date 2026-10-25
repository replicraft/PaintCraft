//! Loss consent retains decoded source pixels, never a published replacement.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::registry::Frontend;
use paintcraft_ui::{i18n::import_loss_description, t};

pub(super) struct Pending {
    image: image::Decoded,
    path: PathBuf,
    digest: String,
    owner: Uuid,
    base: Arc<paintcraft_document::Document>,
    ticket: (Uuid, u64),
    frontend: Frontend,
    dirty: bool,
}
impl App {
    pub(super) fn stage_import(
        &mut self,
        image: image::Decoded,
        path: PathBuf,
        digest: String,
    ) -> Result<()> {
        if self.pending_import.is_some() {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "An image import is already awaiting confirmation",
            ));
        }
        if image
            .losses
            .iter()
            .any(|loss| import_loss_description(loss).is_none())
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Image import contains an unrecognized loss",
            ));
        }
        self.pending_import = Some(Pending {
            image,
            path,
            digest,
            owner: self.active_tab_id(),
            base: self.editor.document_snapshot(),
            ticket: self.editor.render_key(),
            frontend: self.format_frontend(),
            dirty: self.editor.is_dirty(),
        });
        self.confirm = Some(Confirm::ImportLosses);
        Ok(())
    }
    pub(super) fn resolve_import(&mut self, ctx: &egui::Context, accept: bool) {
        let Some(pending) = self.pending_import.take() else {
            self.confirm = None;
            return;
        };
        self.confirm = None;
        if !accept {
            self.editor.set_status(t!(ctx, "Import cancelled."));
            return;
        }
        if self.active_tab_id() != pending.owner
            || self.editor.render_key() != pending.ticket
            || !Arc::ptr_eq(&self.editor.document_snapshot(), &pending.base)
            || self.format_frontend() != pending.frontend
            || self.has_document_drafts()
            || self.editor.is_dirty() != pending.dirty
            || self.editor.has_pending_edits()
            || self.editor.has_floating_selection()
        {
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let result = (|| {
            let image = pending.image;
            let document = import_image(image.width, image.height, &image.rgba, &pending.digest)?;
            self.install(Editor::with_document(document, None))?;
            self.bound = None;
            self.path_text = pending
                .path
                .with_extension("pcnd")
                .to_string_lossy()
                .into_owned();
            self.editor
                .set_status(format!("Imported {}", pending.path.display()));
            self.recent.remember(&pending.path);
            Ok::<(), Error>(())
        })();
        if let Err(error) = result {
            self.editor.set_status(format!("Cannot open: {error}"));
        }
    }
    pub(super) fn import_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending_import else {
            self.confirm = None;
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("native-import-losses")).show(ctx, |ui| {
            ui.heading(t!(ctx, "Open with these import limitations?"));
            for loss in &pending.image.losses {
                if let Some(description) = import_loss_description(loss) {
                    ui.label(t!(ctx, description));
                }
            }
            ui.horizontal(|ui| {
                accept = ui.button(t!(ctx, "Open image")).clicked();
                cancel = ui.button(t!(ctx, "Cancel")).clicked();
            });
        });
        if ctx.current_pass_index() == 0
            && (accept || cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            self.resolve_import(ctx, accept);
        }
    }
}

#[cfg(test)]
#[path = "importing/tests.rs"]
mod tests;
