//! Import consent belongs to the document/profile captured before the picker.
pub(crate) mod icns;
pub(crate) mod pages;
pub(crate) mod svg;
use crate::{app::WebApp, files::Loaded};
use paintcraft_classic::ClassicVariant;
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use paintcraft_ui::t;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct Owner {
    base: Arc<Document>,
    ticket: Ticket,
    variant: ClassicVariant,
    frontend: paintcraft_ui::actions::Frontend,
    dirty: bool,
}
impl Owner {
    pub fn capture(app: &WebApp) -> Self {
        Self {
            base: app.editor.session.snapshot(),
            ticket: app.editor.session.ticket(),
            variant: app.variant,
            frontend: app.frontend,
            dirty: app.editor.session.is_dirty(),
        }
    }
    pub fn current(&self, app: &WebApp) -> bool {
        Arc::ptr_eq(&self.base, &app.editor.session.snapshot())
            && self.ticket == app.editor.session.ticket()
            && self.variant == app.variant
            && self.frontend == app.frontend
            && self.dirty == app.editor.session.is_dirty()
            && !app.editor.tools.gesture_in_progress()
            && !app.editor.tools.is_editing_text()
            && app.text.preedit.is_empty()
    }
}
#[derive(Debug)]
pub(crate) struct Pending {
    pub loaded: Loaded,
    pub owner: Owner,
}
impl WebApp {
    pub(crate) fn accept_loaded(&mut self, loaded: Loaded, ctx: &eframe::egui::Context) {
        let Some(owner) = self.import_owner.take().filter(|owner| owner.current(self)) else {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        };
        if loaded.losses.is_empty() {
            self.install(loaded);
        } else if loaded
            .losses
            .iter()
            .all(|loss| paintcraft_ui::i18n::import_loss_description(loss).is_some())
        {
            self.pending_import = Some(Pending { loaded, owner });
        } else {
            self.message = t!(ctx, "This image contains unsupported import data.");
        }
    }
    pub(crate) fn resolve_import(&mut self, ctx: &eframe::egui::Context, accept: bool) {
        let Some(pending) = self.pending_import.take() else {
            return;
        };
        if !accept {
            self.message = t!(ctx, "Import cancelled.");
            return;
        }
        if !pending.owner.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        self.install(pending.loaded);
    }
}

#[cfg(test)]
mod tests;
