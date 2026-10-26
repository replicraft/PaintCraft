//! The editor controller: one pencil, an explicit editing target, undo/redo and
//! safe save, expressed as typed batch transactions against a [`Session`].
//!
//! Editing gestures collect locally and queue one transaction on release. Nothing
//! is published during a gesture; cancellation changes nothing and a stroke is one
//! undo step. Color-picker gestures queue a read-only sample and update session
//! slots without a document transaction. Actions enter through the [`Outbox`].

#[path = "controller/background.rs"]
pub(crate) mod background;
#[path = "controller/canvas.rs"]
mod canvas;
#[path = "controller/dispatch.rs"]
mod dispatch;
mod files;
mod flatten;
mod floating;
mod floating_feedback;
#[path = "controller/history.rs"]
mod history;
mod image_paste;
#[path = "controller/layers.rs"]
mod layers;
#[path = "controller/paint_queue.rs"]
mod paint_queue;
#[path = "controller/picker.rs"]
mod picker;
#[path = "controller/pointer.rs"]
mod pointer;
#[path = "controller/preview_cache.rs"]
mod preview_cache;
#[path = "controller/selection.rs"]
mod selection;
use paintcraft_tools::commands as raster_commands;
pub(crate) use preview_cache::Stamp as PreviewStamp;
pub(crate) use strokes::PreviewPreparation;
#[path = "controller/residency.rs"]
mod residency;
#[path = "controller/retry.rs"]
mod retry;
#[path = "controller/sampled_brush.rs"]
mod sampled_brush;
#[path = "controller/shape_mask.rs"]
mod shape_mask;
#[path = "controller/strokes.rs"]
mod strokes;
#[path = "controller/text.rs"]
mod text;
#[path = "controller/transactions.rs"]
mod transactions;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use paintcraft_commands::Batch;
use paintcraft_core::types::json::Value;
use paintcraft_core::types::{Result, Rgba};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::execute::Selection;
use paintcraft_editor::session::Session;
use paintcraft_formats::recovery::{Checkpoint, Recovery, RecoveryToken};
use paintcraft_formats::{pcnd, save};
use uuid::Uuid;

use paintcraft_ui::outbox::{Outbox, Token};

use paintcraft_tools::ToolState;
pub use paintcraft_tools::{Action, Preview, ShapeStyle, Tool, ToolOptions};

/// A layer row for the layer list, topmost first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRow {
    pub id: Uuid,
    pub name: String,
    pub visible: bool,
}

#[derive(Debug)]
pub struct Editor {
    session: Session,
    canvas_size_edit: Option<(u32, u32)>,
    tools: ToolState,
    pub(crate) adopted_brush: Option<paintcraft_formats::abr::adoption::AdoptedPreset>,
    floating_source: Option<floating::Source>,
    image_paste: Option<image_paste::Placing>,
    floating_commit_on_release: bool,
    text_renderer: Option<crate::text::Shared>,
    target: Option<Uuid>,
    outbox: Outbox<Action>,
    actions: std::collections::VecDeque<background::Queued>,
    file_barrier: Option<Uuid>,
    editing: Option<background::Service>,
    pending_edit: Option<background::Pending>,
    waiting_dispatch: Option<dispatch::Waiting>,
    waiting_edit: Option<retry::Waiting>,
    maintenance: Option<background::Pending>,
    residency_attempt: Option<((Uuid, u64), std::time::Instant)>,
    publication_preview: Option<Preview>,
    preview_lineage: Uuid,
    preview_queue_revision: paintcraft_tools::PreviewRevision,
    preview_cache: preview_cache::Cache,
    path: Option<PathBuf>,
    destination: Option<crate::destination::Binding>,
    status: String,
    recovery: Option<Arc<Recovery>>,
    checkpointed: Option<(Uuid, u64)>,
    checkpoint_info: Option<Checkpoint>,
    save_outcome: Option<bool>,
}

fn layer_ref(id: Uuid) -> Value {
    Value::object([("id", Value::str(id.to_string()))])
}

fn point(p: (i64, i64)) -> Value {
    Value::object([("x", Value::Int(p.0)), ("y", Value::Int(p.1))])
}

impl Editor {
    /// A new white document with one `Background` layer as the editing target.
    pub fn new_document(width: u32, height: u32) -> Result<Editor> {
        // Native New creates an independent document. The CLI retains its explicit
        // deterministic seed contract; desktop sessions use a fresh valid seed.
        let seed = (Uuid::new_v4().as_u128() as u64) & 9_007_199_254_740_991;
        let doc = new_document(
            width,
            height,
            Some(Rgba([255, 255, 255, 255])),
            seed,
            POLICY,
        )?;
        let mut e = Editor::with_document(doc, None);
        e.status = format!("New {width}×{height} document");
        Ok(e)
    }

    /// An editor over an existing document and optional authorized destination.
    /// Use `open` for an existing file so its acquired content is guarded.
    pub fn with_document(doc: Document, path: Option<PathBuf>) -> Editor {
        let target = doc.roots.last().copied();
        let mut session = Session::new(doc);
        if let Ok(store) = paintcraft_platform::spill::NativeSpillStore::new(
            &std::env::temp_dir().join("paintcraft-history"),
            8 << 30,
            4 << 20,
        ) {
            session.set_history_spill_store(Arc::new(store));
        }
        Editor {
            session,
            canvas_size_edit: None,
            tools: ToolState::default(),
            floating_source: None,
            adopted_brush: None,
            image_paste: None,
            floating_commit_on_release: true,
            text_renderer: None,
            target,
            outbox: Outbox::default(),
            actions: Default::default(),
            file_barrier: None,
            editing: None,
            pending_edit: None,
            waiting_dispatch: None,
            waiting_edit: None,
            maintenance: None,
            residency_attempt: None,
            publication_preview: None,
            preview_lineage: Uuid::new_v4(),
            preview_queue_revision: Default::default(),
            preview_cache: Default::default(),
            destination: path
                .as_ref()
                .map(|_| crate::destination::Binding::authorized()),
            path,
            status: String::new(),
            recovery: None,
            checkpointed: None,
            checkpoint_info: None,
            save_outcome: None,
        }
    }

    /// Open work recovered from a checkpoint. It counts as unsaved, and it is not
    /// bound to any file until the user saves it.
    pub fn recover(checkpoint: &Path) -> Result<Editor> {
        Ok(Self::recovered_document(Recovery::read_checkpoint(
            checkpoint,
        )?))
    }

    pub(crate) fn recovered_document(document: Document) -> Self {
        let mut editor = Self::with_document(document, None);
        editor.session.mark_unsaved();
        editor.status = "Recovered unsaved work".into();
        editor
    }

    /// Start protecting this editor's work with crash-recovery checkpoints.
    pub fn attach_recovery(&mut self, recovery: Recovery) {
        self.attach_shared_recovery(Arc::new(recovery));
    }
    pub(crate) fn attach_shared_recovery(&mut self, recovery: Arc<Recovery>) {
        self.recovery = Some(recovery);
        self.checkpointed = None;
        self.checkpoint_info = None;
    }

    /// Write a checkpoint if there is unsaved work newer than the last one.
    /// Skipped while a gesture is in progress. Returns whether one was written.
    pub fn autosave(&mut self) -> bool {
        let key = self.render_key();
        if !self.is_dirty() {
            if self.checkpoint_info.is_some()
                && let Err(error) = self.discard_checkpoint()
            {
                self.status = format!("Recovery cleanup failed: {error}");
            }
            return false;
        }
        if self.has_pending_edits() || self.gesture_in_progress() || self.checkpointed == Some(key)
        {
            return false;
        }
        let Some(recovery) = &self.recovery else {
            return false;
        };
        let result = recovery.checkpoint_document(self.session.document(), self.path.as_deref());
        match result {
            Ok(info) => {
                self.checkpointed = Some(key);
                if !info.warnings.is_empty() {
                    self.status =
                        format!("Recovery checkpoint saved; {}", info.warnings.join("; "));
                }
                self.checkpoint_info = Some(info);
                true
            }
            Err(e) => {
                self.status = format!("Autosave failed: {e}");
                false
            }
        }
    }

    pub(crate) fn recovery_handle(&self) -> Option<Arc<Recovery>> {
        self.recovery.clone()
    }
    pub(crate) fn recovery_candidate(&self) -> Option<((Uuid, u64), Arc<Recovery>)> {
        let key = self.render_key();
        if self.has_pending_edits()
            || self.gesture_in_progress()
            || !self.is_dirty()
            || self.checkpointed == Some(key)
        {
            return None;
        }
        Some((key, self.recovery_handle()?))
    }
    pub(crate) fn accept_checkpoint(
        &mut self,
        token: RecoveryToken,
        key: (Uuid, u64),
        info: Checkpoint,
    ) {
        if self
            .recovery
            .as_ref()
            .is_none_or(|recovery| !recovery.is_current(token))
            || info.state_id != key.0
        {
            return;
        }
        self.checkpointed = Some(key);
        if !info.warnings.is_empty() {
            self.status = format!("Recovery checkpoint saved; {}", info.warnings.join("; "));
        }
        self.checkpoint_info = Some(info);
    }
    pub(crate) fn forget_checkpoint(&mut self) {
        self.checkpointed = None;
        self.checkpoint_info = None;
    }

    /// Cap the memory held by the document and its undo history.
    pub fn set_history_budget(&mut self, bytes: u64) {
        self.session.set_history_budget(bytes);
    }

    pub fn undo_depth(&self) -> usize {
        self.session.undo_depth()
    }
    pub fn redo_depth(&self) -> usize {
        self.session.redo_depth()
    }

    /// The user chose to throw this work away: its checkpoint must not come back.
    pub fn discard_checkpoint(&mut self) -> Result<()> {
        self.checkpoint_info = None;
        self.checkpointed = None;
        if let Some(r) = &self.recovery {
            r.clear()?;
        }
        Ok(())
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    /// Whether unsaved work has a confirmed recovery checkpoint; later edits may be newer.
    pub fn has_checkpoint(&self) -> bool {
        self.checkpointed.is_some() && self.is_dirty()
    }

    pub fn checkpoint_info(&self) -> Option<&Checkpoint> {
        self.checkpoint_info.as_ref()
    }

    pub fn document_snapshot(&self) -> std::sync::Arc<Document> {
        self.session.snapshot()
    }

    pub fn document(&self) -> &Document {
        self.session.document()
    }

    pub fn selection(&self) -> Selection {
        self.session.selection()
    }

    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty() || self.has_pending_edits() || self.has_image_paste()
    }

    /// Number of publications so far (commits, undo and redo each count).
    pub fn publications(&self) -> u64 {
        self.session.ticket().publication_epoch
    }

    /// Increases whenever the visible document may have changed.
    pub fn render_key(&self) -> (Uuid, u64) {
        let t = self.session.ticket();
        (t.state_id, t.publication_epoch)
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn take_save_outcome(&mut self) -> Option<bool> {
        self.save_outcome.take()
    }

    pub fn target(&self) -> Option<Uuid> {
        self.target
    }

    /// Choose the editing target explicitly; it must be an existing layer.
    pub fn set_target(&mut self, id: Uuid) -> bool {
        if self.has_image_paste() {
            return false;
        }
        let ok = self.document().layers.contains_key(&id);
        if ok {
            self.cancel_gesture();
            self.target = Some(id);
        }
        ok
    }

    /// The properties of one layer, for the layer editor.
    pub fn layer_properties(&self, id: Uuid) -> Option<(String, u8, usize)> {
        let doc = self.document();
        let l = doc.layers.get(&id)?;
        Some((
            l.name.clone(),
            l.opacity,
            doc.roots.iter().position(|r| *r == id)?,
        ))
    }

    pub fn layers(&self) -> Vec<LayerRow> {
        let doc = self.document();
        doc.roots
            .iter()
            .rev()
            .map(|id| LayerRow {
                id: *id,
                name: doc.layers[id].name.clone(),
                visible: doc.layers[id].visible,
            })
            .collect()
    }

    pub fn gesture_in_progress(&self) -> bool {
        self.has_image_paste() || self.tools.gesture_in_progress()
    }
    pub fn is_editing_text(&self) -> bool {
        self.tools.is_editing_text()
    }
    pub fn type_text(&mut self, typed: &str) {
        let skipped = self.tools.type_text(typed);
        if skipped > 0 {
            self.status = format!("Skipped {skipped} character(s) the built-in font cannot draw");
        }
    }
    pub fn backspace(&mut self) {
        self.tools.backspace();
    }
    pub fn commit_text(&mut self) {
        if let Some((token, action)) = self.tools.commit_text()
            && self.outbox.push(token, action)
        {
            self.preview_queue_revision.advance();
        }
    }
    pub fn cancel_text(&mut self) {
        self.tools.cancel_text();
    }
    pub fn select_tool(&mut self, tool: Tool) {
        if self.has_image_paste() {
            self.status = "Finish placing the pasted image before editing or saving.".into();
            return;
        }
        if !matches!(tool, Tool::Magnifier | Tool::Pan)
            && !(matches!(self.tool, Tool::Magnifier | Tool::Pan) && tool == Tool::Text)
        {
            self.commit_text();
        }
        self.finish_selection_move();
        self.cancel_gesture();
        self.tools.choose_tool(tool);
    }
    fn collect_preview(&self) -> Option<Preview> {
        self.paint_preview()
            .or_else(|| self.queued_move_preview())
            .or_else(|| {
                let published = self
                    .publication_preview
                    .as_ref()
                    .filter(|preview| self.move_preview_current(preview))
                    .cloned();
                if self.editing.is_some() && self.painting_blocked() {
                    published
                } else {
                    self.floating_preview().or(published)
                }
            })
    }
    pub fn gesture_points(&self) -> &[(i64, i64)] {
        self.tools.gesture_points()
    }
    // ---- commands ---------------------------------------------------------------

    /// Consume completions and execute queued activations in arrival order.
    /// Native editors prepare one transaction at a time on the shared worker.
    pub fn run_pending(&mut self) -> u64 {
        self.guard_image_paste();
        self.guard_floating_source();
        self.poll_residency_maintenance();
        if self.maintenance.is_some() {
            return 0;
        }
        let before = self.publications();
        self.poll_edit();
        self.retry_waiting_dispatch();
        self.retry_waiting_edit();
        if self.waiting_edit.is_some() || self.waiting_dispatch.is_some() {
            return self.publications() - before;
        }
        if !self.outbox.is_empty() {
            self.preview_queue_revision.advance();
        }
        self.actions.extend(
            self.outbox
                .drain()
                .into_iter()
                .map(background::Queued::Edit),
        );
        if self.file_barrier.is_some() {
            return self.publications() - before;
        }
        while !self.has_inflight_edit() {
            if let Some(background::Queued::FileBarrier(_)) = self.actions.front() {
                if let Some(background::Queued::FileBarrier(id)) = self.actions.pop_front() {
                    self.file_barrier = Some(id);
                    self.preview_queue_revision.advance();
                }
                break;
            }
            if self
                .editing
                .as_ref()
                .is_some_and(|service| !service.available())
            {
                if self.publication_preview.is_none() {
                    self.publication_preview =
                        self.actions.front().and_then(|action| match action {
                            background::Queued::Edit(action) => background::preview(action),
                            _ => None,
                        });
                    if self.publication_preview.is_some() {
                        self.preview_queue_revision.advance();
                    }
                }
                break;
            }
            let Some(background::Queued::Edit(action)) = self.actions.pop_front() else {
                break;
            };
            self.preview_queue_revision.advance();
            self.publication_preview = self
                .editing
                .as_ref()
                .and_then(|_| background::preview(&action));
            self.execute(action);
            if !self.has_inflight_edit() {
                self.publication_preview = None;
            }
        }
        self.publications() - before
    }

    fn execute(&mut self, action: Action) {
        match action {
            Action::MoveSelection {
                layer,
                offset,
                clear,
                transparent_key,
            } => self.move_selection(layer, offset, clear, transparent_key),
            Action::SelectPolygon { points } => self.select_polygon(&points),
            Action::InvertSelection => self.invert_selection(),
            Action::PickColor(pick) => self.pick_color(pick),
            Action::CropToSelection => self.crop_to_selection(),
            Action::EraseSelection { color } => self.erase_selection(color),
            Action::TransformImage { transform, clear } => self.transform_image(transform, clear),
            Action::Contour(contour) => self.contour(&contour),
            Action::ResizeCanvas { width, height } => self.resize_canvas(width, height),
            Action::ResizeCanvasAnchored {
                width,
                height,
                anchor,
            } => {
                let ops = paintcraft_tools::commands::resize_canvas_anchored(
                    self.document(),
                    width,
                    height,
                    anchor,
                );
                if !ops.is_empty() {
                    self.draw(ops, "Canvas size");
                }
            }
            Action::ClearCanvas { color } => self.clear_canvas(color),
            Action::FlattenImage => self.flatten_image(),
            Action::Stroke(stroke) => self.stroke(&stroke),
            Action::ToolFailure(message) => self.status = message.into(),
            Action::SampledStroke(stroke) => self.sampled_stroke(stroke),
            Action::Fill {
                layer,
                at,
                color,
                tolerance,
            } => {
                let op = Value::object([
                    ("op", Value::str("raster.flood_fill")),
                    ("layer_ref", layer_ref(layer)),
                    ("at", point(at)),
                    ("color", Value::str(color.to_hex())),
                    ("mode", Value::str("source-over")),
                    ("tolerance", Value::Int(i64::from(tolerance))),
                ]);
                self.draw(vec![op], "Fill");
            }
            Action::Shape {
                layer,
                tool,
                from,
                to,
                stroke,
                fill,
            } => {
                self.draw(
                    vec![raster_commands::shape(layer, tool, from, to, stroke, fill)],
                    tool.name(),
                );
            }
            Action::Text {
                style,
                background,
                layer,
                at,
                text,
                scale,
                color,
            } => {
                if let Some(style) = style {
                    self.draw_font_text(layer, at, &text, &style, color, background);
                    return;
                }
                self.draw(
                    vec![raster_commands::bitmap_text(layer, at, &text, scale, color)],
                    "Text",
                );
            }
            Action::Select { corners } => {
                let op = match corners {
                    Some((a, b)) => Value::object([
                        ("op", Value::str("selection.set_rect")),
                        (
                            "rect",
                            Value::object([
                                ("x", Value::Int(a.0.min(b.0))),
                                ("y", Value::Int(a.1.min(b.1))),
                                ("width", Value::Int((a.0 - b.0).abs() + 1)),
                                ("height", Value::Int((a.1 - b.1).abs() + 1)),
                            ]),
                        ),
                    ]),
                    None => Value::object([("op", Value::str("selection.clear"))]),
                };
                self.commit(vec![op], "Selection changed");
            }
            Action::AddLayer => self.add_layer(),
            Action::DuplicateLayer { layer } => self.duplicate_layer(layer),
            Action::DeleteLayer { layer } => {
                let op = Value::object([
                    ("op", Value::str("layer.delete")),
                    ("layer_ref", layer_ref(layer)),
                ]);
                self.draw(vec![op], "Layer deleted");
                self.retarget();
            }
            Action::MoveLayer { layer, to_index } => {
                let op = Value::object([
                    ("op", Value::str("layer.reorder")),
                    ("layer_ref", layer_ref(layer)),
                    ("to_index", Value::Int(to_index as i64)),
                ]);
                self.draw(vec![op], "Layer moved");
            }
            Action::SetLayerProperties {
                layer,
                name,
                visible,
                opacity,
            } => {
                let op = Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("name", Value::str(name)),
                    ("visible", Value::Bool(visible)),
                    ("opacity_u8", Value::Int(i64::from(opacity))),
                ]);
                self.draw(vec![op], "Layer properties changed");
            }
            Action::Rename { layer, name } => {
                let op = Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("name", Value::str(name)),
                ]);
                self.draw(vec![op], "Layer renamed");
            }
            Action::SetOpacity { layer, opacity } => {
                let op = Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("opacity_u8", Value::Int(i64::from(opacity))),
                ]);
                self.draw(vec![op], "Layer opacity changed");
            }
            Action::SetVisible { layer, visible } => {
                let op = Value::object([
                    ("op", Value::str("layer.set_properties")),
                    (
                        "layer_ref",
                        Value::object([("id", Value::str(layer.to_string()))]),
                    ),
                    ("visible", Value::Bool(visible)),
                ]);
                self.commit(vec![op], "Layer visibility changed");
            }
            Action::Undo => {
                self.cancel_gesture();
                if self.start_navigation(background::NavigationRequest::Undo) {
                    return;
                }
                self.status = match self.session.undo() {
                    Ok(Some(_)) => "Undo".into(),
                    Ok(None) => "Nothing to undo".into(),
                    Err(error) => format!("Cannot undo: {error}"),
                };
                self.retarget();
            }
            Action::Redo => {
                self.cancel_gesture();
                if self.start_navigation(background::NavigationRequest::Redo) {
                    return;
                }
                self.status = match self.session.redo() {
                    Ok(Some(_)) => "Redo".into(),
                    Ok(None) => "Nothing to redo".into(),
                    Err(error) => format!("Cannot redo: {error}"),
                };
                self.retarget();
            }
            Action::NavigateHistory {
                owner,
                state_id,
                epoch,
                history_id,
            } => self.navigate_history(owner, state_id, epoch, history_id),
            Action::Save => self.save(),
        }
    }

    /// Keep the target valid after history navigation.
    fn retarget(&mut self) {
        if !self
            .target
            .is_some_and(|t| self.document().layers.contains_key(&t))
        {
            self.target = self.document().roots.last().copied();
        }
    }

    pub(crate) fn acknowledge_saved_state(&mut self, state: Uuid) {
        self.session.mark_saved_state(state);
    }
    pub(crate) fn invalidate_saved_state(&mut self) {
        self.session.mark_unsaved();
    }

    pub(crate) fn save_captured(&mut self) {
        if self.can_capture_file() {
            self.save();
        }
    }
}

impl std::ops::Deref for Editor {
    type Target = ToolState;
    fn deref(&self) -> &ToolState {
        &self.tools
    }
}
impl std::ops::DerefMut for Editor {
    fn deref_mut(&mut self) -> &mut ToolState {
        &mut self.tools
    }
}

#[cfg(test)]
mod tests;
