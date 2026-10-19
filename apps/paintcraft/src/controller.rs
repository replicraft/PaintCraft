//! The editor controller: one pencil, an explicit editing target, undo/redo and
//! safe save, all expressed as batch v1 transactions against a [`Session`].
//!
//! A gesture (pointer down, moves, up) is collected locally and committed as
//! exactly one transaction on release. Nothing is published while it is in
//! progress, so a cancelled or interrupted gesture changes nothing and a stroke
//! is one undo step. Commands reach the session only through the [`Outbox`].

#[path = "controller/history.rs"]
mod history;
#[path = "controller/strokes.rs"]
mod strokes;

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

/// Points per `raster.pencil_path` operation (the schema maximum).
const POINTS_PER_OP: usize = 65_536;
/// Operations per batch (the schema maximum); longer gestures are cut off.
const OPS_PER_BATCH: usize = 1024;

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
    tools: ToolState,
    target: Option<Uuid>,
    outbox: Outbox<Action>,
    path: Option<PathBuf>,
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
        let doc = new_document(width, height, Some(Rgba([255, 255, 255, 255])), 0, POLICY)?;
        let mut e = Editor::with_document(doc, None);
        e.status = format!("New {width}×{height} document");
        Ok(e)
    }

    pub fn open(path: &Path) -> Result<Editor> {
        let bytes = std::fs::read(path).map_err(|e| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::StorageFailed,
                format!("cannot read file: {}", e.kind()),
            )
        })?;
        let mut e = Editor::with_document(pcnd::read(&bytes)?, Some(path.to_path_buf()));
        e.status = format!("Opened {}", path.display());
        Ok(e)
    }

    /// An editor over an existing document, optionally bound to a file.
    pub fn with_document(doc: Document, path: Option<PathBuf>) -> Editor {
        let target = doc.roots.last().copied();
        Editor {
            session: Session::new(doc),
            tools: ToolState::default(),
            target,
            outbox: Outbox::default(),
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
        if self.tools.gesture_in_progress() || self.checkpointed == Some(key) {
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
        if self.tools.gesture_in_progress() || !self.is_dirty() || self.checkpointed == Some(key) {
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
        self.session.is_dirty()
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

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    pub(crate) fn restore_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub(crate) fn take_save_outcome(&mut self) -> Option<bool> {
        self.save_outcome.take()
    }

    pub fn target(&self) -> Option<Uuid> {
        self.target
    }

    /// Choose the editing target explicitly; it must be an existing layer.
    pub fn set_target(&mut self, id: Uuid) -> bool {
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
        self.tools.gesture_in_progress()
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
        if let Some((token, action)) = self.tools.commit_text() {
            self.outbox.push(token, action);
        }
    }
    pub fn cancel_text(&mut self) {
        self.tools.cancel_text();
    }
    pub fn select_tool(&mut self, tool: Tool) {
        self.commit_text();
        self.cancel_gesture();
        self.tools.tool = tool;
    }
    pub fn preview(&self) -> Option<Preview> {
        self.tools.preview()
    }
    pub fn gesture_points(&self) -> &[(i64, i64)] {
        self.tools.gesture_points()
    }
    pub fn pointer_down(&mut self, point: (i64, i64)) {
        if self.tools.gesture_in_progress() {
            return;
        }
        self.commit_text();
        let layer = match (
            self.tool,
            self.target
                .filter(|id| self.document().layers.contains_key(id)),
        ) {
            (Tool::Select, t) => t.unwrap_or_else(Uuid::nil),
            (_, Some(layer)) => layer,
            (_, None) => {
                self.status = "No editing target: add or choose a layer".into();
                return;
            }
        };
        if self.tool != Tool::Select && !self.document().layers[&layer].visible {
            self.status = "The target layer is hidden; show it or choose another layer".into();
            return;
        }
        if self.tool == Tool::Text {
            self.tools.begin_text(layer, point);
            self.status =
                "Type your text; click elsewhere or press Ctrl+Enter to place it, Escape to cancel"
                    .into();
        } else {
            self.tools.pointer_down(layer, point);
        }
    }
    pub fn pointer_move(&mut self, point: (i64, i64)) {
        self.tools.pointer_move(point);
    }
    pub fn pointer_up(&mut self) {
        if let Some((token, action)) = self.tools.pointer_up() {
            self.outbox.push(token, action);
        }
    }
    pub fn cancel_gesture(&mut self) {
        self.tools.cancel_gesture();
    }
    pub fn pointer_event(&mut self, event: paintcraft_input::PointerEvent) {
        use paintcraft_input::PointerEvent;
        match event {
            PointerEvent::Down { at } => self.pointer_down(at),
            PointerEvent::Move { at } => self.pointer_move(at),
            PointerEvent::Up => self.pointer_up(),
            PointerEvent::Cancel => self.cancel_gesture(),
        }
    }

    // ---- commands ---------------------------------------------------------------

    /// Queue a command for one user activation. `token` must be stable across
    /// layout passes of the same activation and different between activations.
    pub fn request(&mut self, token: Token, action: Action) -> bool {
        // Typed text is part of the document the user is about to undo, save or change.
        if !matches!(action, Action::Text { .. }) {
            self.commit_text();
        }
        self.outbox.push(token, action)
    }

    /// Execute everything queued, once. Returns how many publications resulted.
    pub fn run_pending(&mut self) -> u64 {
        let before = self.publications();
        for action in self.outbox.drain() {
            self.execute(action);
        }
        self.publications() - before
    }

    fn execute(&mut self, action: Action) {
        match action {
            Action::Stroke {
                layer,
                points,
                color,
                size,
                round,
            } => self.stroke(layer, &points, color, size, round),
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
                let mut pairs = vec![
                    ("op", Value::str("raster.shape")),
                    ("layer_ref", layer_ref(layer)),
                    (
                        "kind",
                        Value::str(match tool {
                            Tool::Line => "line",
                            Tool::Rect => "rect",
                            _ => "ellipse",
                        }),
                    ),
                    ("from", point(from)),
                    ("to", point(to)),
                    ("mode", Value::str("source-over")),
                ];
                if let Some((c, size)) = stroke {
                    pairs.push((
                        "stroke",
                        Value::object([
                            ("color", Value::str(c.to_hex())),
                            ("size", Value::Int(i64::from(size))),
                        ]),
                    ));
                }
                if let Some(c) = fill {
                    pairs.push(("fill", Value::str(c.to_hex())));
                }
                let op = Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect());
                self.draw(vec![op], tool.name());
            }
            Action::Text {
                layer,
                at,
                text,
                scale,
                color,
            } => {
                let op = Value::object([
                    ("op", Value::str("raster.text")),
                    ("layer_ref", layer_ref(layer)),
                    ("at", point(at)),
                    ("text", Value::str(text)),
                    ("scale", Value::Int(i64::from(scale))),
                    ("color", Value::str(color.to_hex())),
                    ("mode", Value::str("source-over")),
                ]);
                self.draw(vec![op], "Text");
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
                self.status = if self.session.undo().is_some() {
                    "Undo".into()
                } else {
                    "Nothing to undo".into()
                };
                self.retarget();
            }
            Action::Redo => {
                self.cancel_gesture();
                self.status = if self.session.redo().is_some() {
                    "Redo".into()
                } else {
                    "Nothing to redo".into()
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

    fn commit(
        &mut self,
        operations: Vec<Value>,
        done: &str,
    ) -> Option<paintcraft_editor::session::Receipt> {
        let t = self.session.ticket();
        let batch = Value::object([
            ("schema", Value::str("paintcraft.batch/v2")),
            ("atomic", Value::Bool(true)),
            ("policy", Value::str(POLICY)),
            (
                "base",
                Value::object([
                    ("state_id", Value::str(t.state_id.to_string())),
                    (
                        "publication_epoch",
                        Value::str(t.publication_epoch.to_string()),
                    ),
                ]),
            ),
            ("operations", Value::Array(operations)),
        ]);
        match Batch::from_value(&batch).and_then(|b| self.session.apply(&b)) {
            Ok(r) => {
                self.status = done.to_owned();
                if self.session.last_trimmed() > 0 {
                    // History is only ever shortened openly, never silently.
                    self.status += &format!(
                        " — the {} oldest undo step(s) were dropped to stay within the memory budget",
                        self.session.last_trimmed()
                    );
                }
                Some(r)
            }
            Err(e) => {
                self.status = format!("Edit failed: {e}");
                None
            }
        }
    }

    /// Commit drawing operations; a batch that changes nothing is reported as such.
    fn draw(&mut self, ops: Vec<Value>, name: &str) {
        if let Some(r) = self.commit(ops, name)
            && !r.committed
        {
            self.status = format!("{name} changed nothing");
        }
    }

    fn add_layer(&mut self) {
        let doc = self.document();
        let (w, h) = doc.canvas;
        let name = format!("Layer {}", doc.roots.len() + 1);
        let op = Value::object([
            ("op", Value::str("layer.add")),
            ("ref", Value::str("layer")),
            ("name", Value::str(&name)),
            ("width", Value::Int(i64::from(w))),
            ("height", Value::Int(i64::from(h))),
            (
                "origin",
                Value::object([("x", Value::Int(0)), ("y", Value::Int(0))]),
            ),
            ("default_pixel", Value::str("#00000000")),
        ]);
        if let Some(r) = self.commit(vec![op], &format!("Added {name}")) {
            // The new layer becomes the target only after a successful commit.
            self.target = r.aliases.get("layer").copied().or(self.target);
        }
    }

    pub(crate) fn acknowledge_saved_state(&mut self, state: Uuid) {
        self.session.mark_saved_state(state);
    }

    fn save(&mut self) {
        self.save_outcome = Some(false);
        let Some(path) = self.path.clone() else {
            self.status = "Choose a file name before saving".into();
            return;
        };
        let outcome = save::save_with(&path, true, save::STAGED_FILE_BYTES_MAX, |sink| {
            pcnd::write_to(self.document(), sink)
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
