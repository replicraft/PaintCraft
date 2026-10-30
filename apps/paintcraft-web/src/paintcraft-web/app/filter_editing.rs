//! Studio intentions, private parameter ownership and asynchronous display state.
use super::*;
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_studio::filters::{self, Action as Intent, ParameterResponse, Parameters};
use paintcraft_tools::FilterEdit;
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
mod bridge;
pub(crate) mod render;
#[cfg(any(target_arch = "wasm32", test))]
mod snapshot;
mod state;
mod view;
use render::{Capture, Renderer, Request, View};
use state::{Draft, Revision};
#[cfg(test)]
mod late_base_tests;
#[cfg(test)]
mod mask_tests;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct Display {
    renderer: Renderer,
    source: Option<Capture>,
    committed: Option<Capture>,
    texture: Option<egui::TextureHandle>,
    footprint: Option<paintcraft_core::types::Rect>,
    view: Option<View>,
}
pub(super) struct State {
    draft: Draft,
    base_texture: Option<egui::TextureHandle>,
    base_footprint: Option<paintcraft_core::types::Rect>,
    base_view: Option<View>,
}
impl std::fmt::Debug for Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterDisplay")
            .field("renderer", &self.renderer)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterState")
            .field("draft", &self.draft)
            .finish_non_exhaustive()
    }
}
impl WebApp {
    pub(super) fn filter_private_input(&self) -> bool {
        self.text.is_composing()
            || self.editor.tools.is_editing_text()
            || self.editor.tools.gesture_in_progress()
            || self.editor.tools.floating_selection().is_some()
            || self.paste.busy()
            || self.paste.is_placing()
            || self.clipboard.busy()
    }
    pub(crate) fn filter_edit_active(&self) -> bool {
        self.filter_editing.is_some()
    }
    pub(crate) fn filter_navigation_ready(&self) -> bool {
        self.filter_editing
            .as_ref()
            .is_some_and(|s| s.draft.current(self))
            && !self.modal_blocked()
    }
    pub(crate) fn begin_studio_filter(&mut self, ctx: &egui::Context, intent: Intent<Uuid>) {
        if self.frontend != Frontend::Studio || self.blocked() || self.filter_private_input() {
            return;
        }
        let Some(operation) = state::operation(self, intent) else {
            return;
        };
        if !matches!(intent, Intent::Add(_) | Intent::Edit(_)) {
            let ticket = self.editor.session.ticket();
            let edit = FilterEdit {
                owner: self.editor.session.owner_id(),
                state_id: ticket.state_id,
                publication_epoch: ticket.publication_epoch,
                operation,
            };
            self.edit(Edit::Filter(edit));
            ctx.request_repaint();
            return;
        }
        match Draft::capture(self, operation) {
            Ok(draft) => {
                let (base_texture, base_footprint, base_view) = if self
                    .filter_display
                    .committed
                    .as_ref()
                    .is_some_and(|source| self.filter_source_current(source))
                {
                    (
                        self.filter_display.texture.clone(),
                        self.filter_display.footprint,
                        self.filter_display.view,
                    )
                } else {
                    let texture = self.committed_filter_base_texture();
                    let footprint = texture.as_ref().and(self.canvas.filter_footprint);
                    let view = texture.as_ref().and(self.canvas.filter_view);
                    (texture, footprint, view)
                };
                self.canvas.texture = base_texture.clone();
                self.canvas.filter_footprint = base_footprint;
                self.canvas.invalidate();
                self.filter_editing = Some(State {
                    draft,
                    base_texture,
                    base_footprint,
                    base_view,
                });
                self.queue_filter_preview();
                ctx.request_repaint();
            }
            Err(error) => self.message = format!("Filter not opened: {error}"),
        }
    }
    pub(super) fn queue_filter_preview(&mut self) {
        let Some(state) = &self.filter_editing else {
            return;
        };
        let draft = &state.draft;
        let Some(view) = self.canvas.filter_view else {
            return;
        };
        if !draft.parameters.preview && draft.prepared == Some(draft.revision) {
            if state.base_view != Some(view) {
                self.filter_display.renderer.want(
                    Capture {
                        owner: draft.base.owner,
                        ticket: draft.ticket(),
                        draft: draft.revision.draft,
                        document: draft.source.clone(),
                    },
                    Request {
                        revision: draft.revision,
                        edit: None,
                        render: true,
                        view,
                    },
                );
            }
            return;
        }
        match draft.operation() {
            Ok(edit) => self.filter_display.renderer.want(
                Capture {
                    owner: draft.base.owner,
                    ticket: draft.ticket(),
                    draft: draft.revision.draft,
                    document: draft.source.clone(),
                },
                Request {
                    revision: draft.revision,
                    edit: Some(edit),
                    render: draft.parameters.preview,
                    view,
                },
            ),
            Err(error) => {
                self.message = format!("Filter not applied: {error}");
            }
        }
    }
    pub(crate) fn cancel_filter_edit(&mut self) {
        if let Some(state) = self.filter_editing.take() {
            if state.draft.current(self)
                && let Some(texture) = state.base_texture
            {
                self.canvas.texture = Some(texture);
                self.canvas.filter_footprint = state.base_footprint;
                self.mark_filter_canvas_current();
                self.filter_display.texture = self.canvas.texture.clone();
                self.filter_display.footprint = self.canvas.filter_footprint;
                self.filter_display.view = state.base_view;
                self.filter_display.committed = Some(Capture {
                    owner: state.draft.base.owner,
                    ticket: state.draft.ticket(),
                    draft: state.draft.revision.draft,
                    document: state.draft.source,
                });
            } else {
                self.canvas.invalidate();
            }
            self.filter_display.renderer.reset();
            self.filter_display.source = None;
        }
    }
    pub(crate) fn pump_filter_editing(&mut self, ctx: &egui::Context) {
        if self
            .filter_editing
            .as_ref()
            .is_some_and(|s| !s.draft.current(self))
        {
            self.cancel_filter_edit();
            self.message = "Filter not applied: its document changed".into();
        }
        if let Some(reply) = self.filter_display.renderer.poll(ctx) {
            self.accept_filter_reply(ctx, reply);
        }
        if let Some(state) = &mut self.filter_editing
            && let Some((revision, error)) = &self.filter_display.renderer.error
            && *revision == state.draft.revision
        {
            state.draft.error = Some(error.clone());
            state.draft.apply = None;
        }
        self.finish_filter_apply(ctx);
    }
    fn ordinary_filter_preview(&self) -> bool {
        self.text.is_composing()
            || self.paste.is_placing()
            || self.editor.tools.is_editing_text()
            || self.editor.tools.floating_selection().is_some()
            || (self.editor.tools.gesture_in_progress()
                && !self.editor.tools.gesture_is_selection())
    }
    fn accept_filter_reply(&mut self, ctx: &egui::Context, reply: render::Reply) {
        let private = self.filter_editing.as_ref().is_some_and(|state| {
            state.draft.current(self) && state.draft.revision == reply.request.revision
        });
        let committed = self.filter_editing.is_none()
            && reply.request.edit.is_none()
            && self.filter_display.source.as_ref().is_some_and(|source| {
                self.filter_source_current(source) && source.draft == reply.request.revision.draft
            });
        if !(private || committed) {
            return;
        }
        if let Some(pixels) = reply.pixels {
            if committed {
                // Store this admitted BASE independently. It is not the current
                // brush/text/IME/floating/paste image, even though the source matches.
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [reply.size.0 as usize, reply.size.1 as usize],
                    &pixels,
                );
                let texture =
                    ctx.load_texture("web-filter-base", image, egui::TextureOptions::NEAREST);
                self.filter_display.committed = self.filter_display.source.clone();
                self.filter_display.texture = Some(texture.clone());
                self.filter_display.footprint = Some(reply.request.view.footprint);
                self.filter_display.view = Some(reply.request.view);
                if !self.ordinary_filter_preview() {
                    self.canvas.texture = Some(texture);
                    self.canvas.filter_footprint = self.filter_display.footprint;
                    self.mark_filter_canvas_current();
                }
            } else {
                self.install_filter_pixels(ctx, reply.size, reply.request.view.footprint, &pixels);
                if let Some(state) = &mut self.filter_editing
                    && !state.draft.parameters.preview
                    && reply.request.edit.is_none()
                {
                    state.base_texture = self.canvas.texture.clone();
                    state.base_footprint = self.canvas.filter_footprint;
                    state.base_view = Some(reply.request.view);
                }
            }
        } else if let Some(state) = &mut self.filter_editing
            && reply.request.edit.is_some()
        {
            state.draft.prepared = Some(state.draft.revision);
        }
    }
    pub(super) fn finish_filter_apply(&mut self, ctx: &egui::Context) {
        let Some(state) = &self.filter_editing else {
            return;
        };
        if state.draft.apply != Some(state.draft.revision) || !state.draft.current(self) {
            return;
        }
        let Ok(edit) = state.draft.operation() else {
            return;
        };
        let Some(view) = self.canvas.filter_view else {
            return;
        };
        let request = Request {
            revision: state.draft.revision,
            edit: Some(edit.clone()),
            render: state.draft.parameters.preview,
            view,
        };
        if state.draft.parameters.preview {
            if !self.filter_display.renderer.ready(&request) {
                return;
            }
        } else if state.draft.prepared != Some(state.draft.revision) {
            return;
        }
        let reuse = state.draft.parameters.preview;
        match self.editor.apply(Edit::Filter(edit)) {
            Ok(()) => {
                self.filter_editing = None;
                self.filter_display.renderer.reset();
                self.filter_display.source = None;
                if reuse {
                    let source = self.filter_capture();
                    self.filter_display.committed = Some(source);
                    self.filter_display.texture = self.canvas.texture.clone();
                    self.filter_display.footprint = self.canvas.filter_footprint;
                    self.filter_display.view = Some(view);
                    self.mark_filter_canvas_current();
                } else {
                    self.canvas.invalidate();
                    self.filter_display.committed = None;
                }
                self.message = "Filter applied".into();
                self.update_title();
            }
            Err(error) => {
                if let Some(state) = &mut self.filter_editing {
                    state.draft.error = Some(error);
                    state.draft.apply = None;
                }
            }
        }
        ctx.request_repaint();
    }
    fn filter_capture(&self) -> Capture {
        Capture {
            owner: self.editor.session.owner_id(),
            ticket: self.editor.session.ticket(),
            draft: Uuid::new_v4(),
            document: self.editor.session.snapshot(),
        }
    }
    fn filter_source_current(&self, source: &Capture) -> bool {
        source.owner == self.editor.session.owner_id()
            && source.ticket == self.editor.session.ticket()
            && std::sync::Arc::ptr_eq(&source.document, &self.editor.session.snapshot())
    }
    pub(crate) fn worker_filter_canvas(&mut self, ctx: &egui::Context) -> bool {
        // Existing ordinary tool previews remain a separate synchronous boundary.
        if self.filter_editing.is_none() && self.ordinary_filter_preview() {
            return false;
        }
        if self.filter_editing.is_some() {
            self.queue_filter_preview();
            self.pump_filter_editing(ctx);
            return true;
        }
        let has_filters = self
            .editor
            .session
            .document()
            .layers
            .values()
            .any(|l| !l.filters.is_empty());
        if !has_filters {
            // A Worker ROI is not a full-canvas texture for the legacy cache key.
            if self.canvas.filter_footprint.is_some() {
                self.canvas.invalidate();
            }
            self.filter_display.renderer.reset();
            self.filter_display.source = None;
            self.pump_filter_editing(ctx);
            return false;
        }
        if self
            .filter_display
            .committed
            .as_ref()
            .is_some_and(|source| self.filter_source_current(source))
        {
            // A valid old-ROI BASE remains honest while the new ROI is pending.
            self.canvas.texture = self.filter_display.texture.clone();
            self.canvas.filter_footprint = self.filter_display.footprint;
            if self.filter_display.view == self.canvas.filter_view {
                self.mark_filter_canvas_current();
                self.pump_filter_editing(ctx);
                return true;
            }
            self.canvas.invalidate();
        } else {
            // Never leave cancelled private pixels displayed as pending BASE.
            self.canvas.texture = None;
            self.canvas.filter_footprint = None;
            self.canvas.invalidate();
        }
        let source = if let Some(source) = &self.filter_display.source
            && self.filter_source_current(source)
        {
            source.clone()
        } else {
            self.filter_capture()
        };
        let Some(view) = self.canvas.filter_view else {
            return true;
        };
        let request = Request {
            revision: Revision {
                draft: source.draft,
                generation: 1,
            },
            edit: None,
            render: true,
            view,
        };
        self.filter_display.renderer.want(source.clone(), request);
        self.filter_display.source = Some(source);
        self.pump_filter_editing(ctx);
        true
    }
    pub(crate) fn detach_committed_filter_texture(&mut self) {
        if self
            .canvas
            .texture
            .as_ref()
            .zip(self.filter_display.texture.as_ref())
            .is_some_and(|(a, b)| a.id() == b.id())
        {
            self.canvas.texture = None;
        }
    }
    pub(crate) fn filter_status(&self) -> Option<String> {
        if self.filter_display.renderer.busy() {
            Some("Updating preview…".into())
        } else {
            self.filter_display
                .renderer
                .error
                .as_ref()
                .map(|(_, e)| format!("Preview unavailable: {e}"))
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn target(operation: &paintcraft_commands::FilterOp) -> Result<Uuid> {
    use paintcraft_commands::{FilterOp, LayerRef};
    let (FilterOp::Add { layer, .. }
    | FilterOp::Update { layer, .. }
    | FilterOp::SetEnabled { layer, .. }
    | FilterOp::Reorder { layer, .. }
    | FilterOp::Remove { layer, .. }
    | FilterOp::Duplicate { layer, .. }
    | FilterOp::Mask { layer, .. }) = operation;
    match layer {
        LayerRef::Id(id) => Ok(*id),
        _ => Err(Error::new(
            ErrorCode::InputSchema,
            "Filter target must be a UUID",
        )),
    }
}
