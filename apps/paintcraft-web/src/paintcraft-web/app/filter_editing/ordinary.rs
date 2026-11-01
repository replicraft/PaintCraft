//! Source6 tool previews are immutable ephemeral sources, never committed BASE.
use super::*;
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use paintcraft_tools::PreviewRevision;
use std::sync::Arc;

#[derive(Clone)]
struct Key {
    owner: Uuid,
    ticket: Ticket,
    source: Arc<Document>,
    tool: PreviewRevision,
    paste: Option<PreviewRevision>,
    composition: Option<crate::text_ui::CompositionKey>,
    fonts: u64,
}
impl Key {
    fn same(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.ticket == other.ticket
            && Arc::ptr_eq(&self.source, &other.source)
            && self.tool == other.tool
            && self.paste == other.paste
            && self.composition == other.composition
            && self.fonts == other.fonts
    }
}
pub(super) struct Transient {
    key: Key,
    // None remembers a failed staging attempt for this exact revision. No retry
    // allocation loop and no speculative history publication or tool cancellation.
    pub(super) capture: Option<Capture>,
}
impl WebApp {
    fn noise_preview_key(&self) -> Key {
        Key {
            owner: self.editor.session.owner_id(),
            ticket: self.editor.session.ticket(),
            source: self.editor.session.snapshot(),
            tool: self.editor.tools.preview_revision(),
            paste: self.paste_revision(),
            composition: self.composition_key(),
            fonts: self.text.font_generation,
        }
    }
    fn restore_noise_preview_base(&mut self) {
        if self
            .filter_display
            .committed
            .as_ref()
            .is_some_and(|s| self.filter_source_current(s))
        {
            self.canvas.texture = self.filter_display.texture.clone();
            self.canvas.filter_footprint = self.filter_display.footprint;
        } else {
            self.canvas.texture = None;
            self.canvas.filter_footprint = None;
        }
        self.canvas.invalidate();
    }
    pub(super) fn forget_noise_preview(&mut self) {
        if self.filter_display.transient.take().is_some() {
            self.filter_display.renderer.reset();
            self.restore_noise_preview_base();
        }
    }
    /// The ordinary old-profile synchronous branch is untouched. Noise sources
    /// instead reuse the one admitted renderer and its bounded desired slot.
    pub(super) fn worker_noise_preview(&mut self, ctx: &egui::Context) -> bool {
        if self.filter_editing.is_some()
            || !self.ordinary_filter_preview()
            || !render::stored_background(self.editor.session.document())
        {
            self.forget_noise_preview();
            return false;
        }
        // Match ordinary canvas staging validity checks, without synchronous
        // appearance evaluation or cancelling a gesture on a Noise scope error.
        self.guard_composition(ctx);
        self.editor.validate_floating();
        self.validate_paste_placement(ctx);
        if !self.ordinary_filter_preview() {
            self.forget_noise_preview();
            return false;
        }
        let key = self.noise_preview_key();
        let changed = self
            .filter_display
            .transient
            .as_ref()
            .is_none_or(|p| !p.key.same(&key));
        if changed {
            self.filter_display.renderer.reset();
            self.restore_noise_preview_base();
            let result = if self.paste.is_placing() {
                self.paste_preview_document()
            } else {
                self.canvas_preview_document()
            };
            let capture = match result {
                Ok(Some(document)) => {
                    let mut capture = self.filter_capture();
                    // The worker authenticates the staged PCND state. The real
                    // publication ticket remains separately frozen in `key`.
                    capture.ticket.state_id = document.state_id;
                    capture.document = Arc::new(document);
                    capture.scoped = true;
                    capture.programs = true;
                    Some(capture)
                }
                Ok(None) => None,
                Err(error) => {
                    self.message = format!("Preview unavailable: {error}");
                    None
                }
            };
            self.filter_display.transient = Some(Transient { key, capture });
        }
        let capture = self
            .filter_display
            .transient
            .as_ref()
            .and_then(|p| p.capture.clone());
        if let (Some(source), Some(view)) = (capture, self.canvas.filter_view) {
            self.filter_display.renderer.want(
                source.clone(),
                Request {
                    revision: Revision {
                        draft: source.draft,
                        generation: 1,
                    },
                    edit: None,
                    render: true,
                    view,
                },
            );
        }
        self.pump_filter_editing(ctx);
        true
    }
    pub(super) fn accept_noise_preview_reply(
        &mut self,
        ctx: &egui::Context,
        reply: &render::Reply,
    ) -> bool {
        let Some(transient) = &self.filter_display.transient else {
            return false;
        };
        let Some(capture) = &transient.capture else {
            return false;
        };
        if capture.draft != reply.request.revision.draft {
            return false;
        }
        // Even a matching in-flight source is obsolete after a tool change,
        // commit/cancel, owner replacement, publication, IME edit, or view move.
        if self.filter_editing.is_none()
            && self.ordinary_filter_preview()
            && transient.key.same(&self.noise_preview_key())
            && reply.request.edit.is_none()
            && reply.request.render
            && reply.request.revision.generation == 1
            && self.canvas.filter_view == Some(reply.request.view)
            && let Some(pixels) = &reply.pixels
        {
            self.install_filter_pixels(ctx, reply.size, reply.request.view.footprint, pixels);
        }
        // Never fall through to committed BASE caching for this transient ID.
        true
    }
}
