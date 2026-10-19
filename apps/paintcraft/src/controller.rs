//! The editor controller: one pencil, an explicit editing target, undo/redo and
//! safe save, all expressed as batch v1 transactions against a [`Session`].
//!
//! A gesture (pointer down, moves, up) is collected locally and committed as
//! exactly one transaction on release. Nothing is published while it is in
//! progress, so a cancelled or interrupted gesture changes nothing and a stroke
//! is one undo step. Commands reach the session only through the [`Outbox`].

use std::path::{Path, PathBuf};

use paintcraft_commands::Batch;
use paintcraft_core::types::json::Value;
use paintcraft_core::types::{Result, Rgba};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::execute::Selection;
use paintcraft_editor::session::Session;
use paintcraft_formats::recovery::Recovery;
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
    recovery: Option<Recovery>,
    checkpointed: Option<(Uuid, u64)>,
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
            save_outcome: None,
        }
    }

    /// Open work recovered from a checkpoint. It counts as unsaved, and it is not
    /// bound to any file until the user saves it.
    pub fn recover(checkpoint: &Path) -> Result<Editor> {
        let bytes = std::fs::read(checkpoint).map_err(|e| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::StorageFailed,
                format!("cannot read the checkpoint: {}", e.kind()),
            )
        })?;
        let mut e = Editor::with_document(pcnd::read(&bytes)?, None);
        e.session.mark_unsaved();
        e.status = "Recovered unsaved work".into();
        Ok(e)
    }

    /// Start protecting this editor's work with crash-recovery checkpoints.
    pub fn attach_recovery(&mut self, recovery: Recovery) {
        self.recovery = Some(recovery);
        self.checkpointed = None;
    }

    /// Write a checkpoint if there is unsaved work newer than the last one.
    /// Skipped while a gesture is in progress. Returns whether one was written.
    pub fn autosave(&mut self) -> bool {
        let key = self.render_key();
        if self.tools.gesture_in_progress() || !self.is_dirty() || self.checkpointed == Some(key) {
            return false;
        }
        let Some(recovery) = &self.recovery else {
            return false;
        };
        let result = pcnd::write(self.session.document())
            .and_then(|bytes| recovery.checkpoint(&bytes, self.path.as_deref()));
        match result {
            Ok(()) => {
                self.checkpointed = Some(key);
                true
            }
            Err(e) => {
                self.status = format!("Autosave failed: {e}");
                false
            }
        }
    }

    /// Cap the memory held by the document and its undo history.
    pub fn set_history_budget(&mut self, bytes: u64) {
        self.session.set_history_budget(bytes);
    }

    pub fn undo_depth(&self) -> usize {
        self.session.undo_depth()
    }

    /// The user chose to throw this work away: its checkpoint must not come back.
    pub fn discard_checkpoint(&mut self) {
        if let Some(r) = &self.recovery {
            r.clear();
        }
        self.checkpointed = None;
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    /// Whether a checkpoint of the current work exists (the document is dirty and was checkpointed).
    pub fn has_checkpoint(&self) -> bool {
        self.checkpointed.is_some() && self.is_dirty()
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

    fn stroke(&mut self, layer: Uuid, points: &[(i64, i64)], color: Rgba, size: u32, round: bool) {
        if points.is_empty() {
            return;
        }
        // Long gestures are split across operations; each starts on the previous
        // chunk's last point so the path stays connected. A translucent colour
        // blends the joint's stamp twice, once per operation.
        let mut ops = Vec::new();
        let mut start = 0;
        while start < points.len() && ops.len() < OPS_PER_BATCH {
            let end = (start + POINTS_PER_OP).min(points.len());
            ops.push(Value::object([
                ("op", Value::str("raster.stroke_path")),
                ("layer_ref", layer_ref(layer)),
                (
                    "points",
                    Value::Array(points[start..end].iter().map(|p| point(*p)).collect()),
                ),
                ("color", Value::str(color.to_hex())),
                (
                    "mode",
                    Value::str(if color.a() == 0 {
                        "replace"
                    } else {
                        "source-over"
                    }),
                ),
                ("size", Value::Int(i64::from(size))),
                ("brush", Value::str(if round { "round" } else { "square" })),
            ]));
            if end == points.len() {
                break;
            }
            start = end - 1;
        }
        self.draw(ops, if color.a() == 0 { "Erase" } else { "Stroke" });
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

    fn save(&mut self) {
        self.save_outcome = Some(false);
        let Some(path) = self.path.clone() else {
            self.status = "Choose a file name before saving".into();
            return;
        };
        let bytes = match pcnd::write(self.document()) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("Not saved: {e}");
                return;
            }
        };
        let outcome = save::save(&path, &bytes, true);
        if outcome.is_success() {
            self.save_outcome = Some(true);
            self.session.mark_saved();
            // The file now holds everything; the checkpoint is obsolete.
            if let Some(r) = &self.recovery {
                r.clear();
            }
            self.checkpointed = None;
            self.status = format!("Saved {}", path.display());
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
mod tests {
    use super::*;

    fn editor() -> Editor {
        let mut e = Editor::new_document(320, 320).unwrap();
        e.request(1, Action::AddLayer);
        e.run_pending();
        e
    }

    fn pixel(e: &Editor, layer: Uuid, x: i64, y: i64) -> Rgba {
        e.document().layers[&layer].raster.get(x, y)
    }

    fn stroke(e: &mut Editor, pts: &[(i64, i64)]) -> u64 {
        e.pointer_down(pts[0]);
        pts[1..].iter().for_each(|p| e.pointer_move(*p));
        e.pointer_up();
        e.run_pending()
    }

    #[test]
    fn a_click_is_exactly_one_transaction() {
        let mut e = editor();
        let layer = e.target().unwrap();
        let (epoch, dirty_before) = (e.publications(), e.is_dirty());
        assert!(dirty_before, "adding a layer already changed the document");
        assert_eq!(stroke(&mut e, &[(5, 5)]), 1);
        assert_eq!(e.publications(), epoch + 1);
        assert_eq!(pixel(&e, layer, 5, 5), Rgba([0, 0, 0, 255]));
        assert_eq!(pixel(&e, layer, 6, 5), Rgba::TRANSPARENT);
    }

    #[test]
    fn a_drag_across_tile_boundaries_is_one_transaction_and_one_undo_step() {
        let mut e = editor();
        let layer = e.target().unwrap();
        let before = e.document().state_id;
        assert_eq!(
            stroke(&mut e, &[(120, 10), (130, 10), (250, 40), (260, 40)]),
            1
        );
        for x in [127, 128, 255, 256] {
            assert_eq!(
                pixel(&e, layer, x, if x < 200 { 10 } else { 40 }).a(),
                255,
                "x = {x}"
            );
        }
        e.request(2, Action::Undo);
        e.run_pending();
        assert_eq!(pixel(&e, layer, 127, 10), Rgba::TRANSPARENT);
        assert_eq!(
            e.document().state_id,
            before,
            "one undo step removes the whole stroke"
        );
        e.request(3, Action::Redo);
        e.run_pending();
        assert_eq!(pixel(&e, layer, 128, 10).a(), 255);
    }

    #[test]
    fn releasing_twice_or_repeating_a_token_does_not_repeat_the_edit() {
        let mut e = editor();
        e.pointer_down((1, 1));
        e.pointer_up();
        e.pointer_up();
        let epoch = e.publications();
        assert_eq!(e.run_pending(), 1);
        assert_eq!(e.publications(), epoch + 1);
        // Two layout passes of one click push the same token.
        assert!(e.request(77, Action::AddLayer));
        assert!(!e.request(77, Action::AddLayer));
        assert_eq!(e.run_pending(), 1);
        assert_eq!(e.layers().len(), 3);
        // A distinct click is a distinct activation.
        e.request(78, Action::AddLayer);
        assert_eq!(e.run_pending(), 1);
    }

    #[test]
    fn a_cancelled_or_interrupted_gesture_publishes_nothing() {
        let mut e = editor();
        let epoch = e.publications();
        e.pointer_down((3, 3));
        e.pointer_move((9, 9));
        e.cancel_gesture();
        e.pointer_up();
        assert_eq!((e.run_pending(), e.publications()), (0, epoch));
        // Undo during a gesture resolves it first.
        e.pointer_down((3, 3));
        e.request(5, Action::Undo);
        assert!(e.gesture_in_progress());
        e.run_pending();
        assert!(!e.gesture_in_progress());
        e.pointer_up();
        assert_eq!(e.run_pending(), 0);
    }

    #[test]
    fn drawing_needs_a_visible_existing_target() {
        let mut e = editor();
        let layer = e.target().unwrap();
        e.request(
            9,
            Action::SetVisible {
                layer,
                visible: false,
            },
        );
        e.run_pending();
        let epoch = e.publications();
        e.pointer_down((1, 1));
        assert!(!e.gesture_in_progress() && e.status().contains("hidden"));
        e.pointer_up();
        assert_eq!((e.run_pending(), e.publications()), (0, epoch));
        assert!(!e.set_target(Uuid::nil()));
        // The explicit choice of another layer draws there, not "wherever".
        let background = e.document().roots[0];
        assert!(e.set_target(background));
        assert_eq!(stroke(&mut e, &[(2, 2)]), 1);
        assert_eq!(pixel(&e, background, 2, 2).a(), 255);
    }

    #[test]
    fn strokes_outside_the_canvas_change_nothing_and_are_reported() {
        let mut e = editor();
        let epoch = e.publications();
        assert_eq!(stroke(&mut e, &[(-50, -50), (-10, -10)]), 0);
        assert_eq!(e.publications(), epoch);
        assert!(e.status().contains("nothing"));
    }

    #[test]
    fn long_gestures_split_into_operations_but_stay_one_transaction() {
        let mut e = editor();
        let layer = e.target().unwrap();
        let pts: Vec<(i64, i64)> = (0..70_000).map(|i| (i - 100, 7)).collect();
        assert_eq!(stroke(&mut e, &pts), 1);
        assert_eq!(pixel(&e, layer, 319, 7).a(), 255);
        assert_eq!(pixel(&e, layer, 0, 8).a(), 0);
    }

    #[test]
    fn save_clears_dirty_only_on_a_confirmed_save() {
        let dir = std::env::temp_dir().join(format!("paintcraft-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = editor();
        stroke(&mut e, &[(10, 10), (40, 40)]);
        e.request(20, Action::Save);
        e.run_pending();
        assert!(e.is_dirty() && e.status().contains("file name"));
        e.set_path(dir.join("missing/a.pcnd"));
        e.request(21, Action::Save);
        e.run_pending();
        assert!(
            e.is_dirty() && e.status().contains("Not saved safely"),
            "{}",
            e.status()
        );
        let file = dir.join("a.pcnd");
        e.set_path(file.clone());
        e.request(22, Action::Save);
        e.run_pending();
        assert!(!e.is_dirty(), "{}", e.status());
        // The saved file reopens to the same document, and undo back past the save is dirty again.
        let reopened = Editor::open(&file).unwrap();
        assert!(reopened.document().same_semantics(e.document()));
        e.request(23, Action::Undo);
        e.run_pending();
        assert!(e.is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    // ---- tools ----------------------------------------------------------------------

    fn with_tool(tool: Tool, f: impl FnOnce(&mut ToolOptions)) -> Editor {
        let mut e = editor();
        e.tool = tool;
        f(&mut e.options);
        e
    }

    #[test]
    fn the_brush_paints_its_size_and_the_pencil_is_always_one_pixel() {
        let mut e = with_tool(Tool::Brush, |o| {
            o.size = 3;
            o.round = false;
        });
        let layer = e.target().unwrap();
        assert_eq!(stroke(&mut e, &[(10, 10)]), 1);
        let covered = (0..320)
            .flat_map(|y| (0..320).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&e, layer, x, y).a() == 255)
            .count();
        assert_eq!(covered, 9);
        e.tool = Tool::Pencil;
        e.options.size = 50;
        assert_eq!(stroke(&mut e, &[(100, 100)]), 1);
        assert_eq!(
            pixel(&e, layer, 101, 100).a(),
            0,
            "size is ignored by the pencil"
        );
    }

    #[test]
    fn the_eraser_clears_to_transparent_in_one_undoable_step() {
        let mut e = with_tool(Tool::Brush, |o| o.size = 1);
        let layer = e.target().unwrap();
        stroke(&mut e, &[(5, 5), (9, 5)]);
        e.tool = Tool::Eraser;
        e.options.size = 3;
        assert_eq!(stroke(&mut e, &[(7, 5)]), 1);
        assert_eq!(pixel(&e, layer, 7, 5), Rgba::TRANSPARENT);
        assert_eq!(pixel(&e, layer, 5, 5).a(), 255);
        assert_eq!(pixel(&e, layer, 9, 5).a(), 255);
        e.request(90, Action::Undo);
        e.run_pending();
        assert_eq!(pixel(&e, layer, 7, 5).a(), 255);
    }

    #[test]
    fn fill_acts_where_the_button_went_down_not_where_it_came_up() {
        let mut e = with_tool(Tool::Fill, |_| {});
        e.color = Rgba([255, 0, 0, 255]);
        let layer = e.target().unwrap();
        e.pointer_down((10, 10));
        e.pointer_move((200, 200));
        e.pointer_up();
        assert_eq!(e.run_pending(), 1);
        assert_eq!(
            pixel(&e, layer, 300, 300),
            Rgba([255, 0, 0, 255]),
            "the whole transparent layer is one region"
        );
        assert!(e.preview().is_none());
    }

    #[test]
    fn shapes_preview_while_dragging_and_publish_once_on_release() {
        for (tool, style, expect_fill) in [
            (Tool::Rect, ShapeStyle::Outline, false),
            (Tool::Rect, ShapeStyle::Filled, true),
            (Tool::Ellipse, ShapeStyle::Both, true),
            (Tool::Line, ShapeStyle::Filled, false),
        ] {
            let mut e = with_tool(tool, |o| {
                o.style = style;
                o.size = 2;
            });
            e.secondary = Rgba([0, 0, 255, 255]);
            let before = e.publications();
            e.pointer_down((20, 20));
            for p in [(30, 25), (40, 40), (60, 50)] {
                e.pointer_move(p);
            }
            let Some(Preview::Shape {
                from,
                to,
                fill,
                stroke,
                ..
            }) = e.preview()
            else {
                panic!("{tool:?} should preview as a shape")
            };
            assert_eq!((from, to), ((20, 20), (60, 50)), "only the two ends matter");
            assert_eq!(fill.is_some(), expect_fill, "{tool:?} {style:?}");
            assert_eq!(
                stroke.is_some(),
                style != ShapeStyle::Filled || tool == Tool::Line
            );
            assert_eq!(e.publications(), before, "nothing is published mid-gesture");
            e.pointer_up();
            assert_eq!(e.run_pending(), 1, "{tool:?}");
        }
    }

    #[test]
    fn a_filled_rectangle_uses_the_primary_colour_and_both_uses_the_secondary_for_the_fill() {
        let mut e = with_tool(Tool::Rect, |o| {
            o.style = ShapeStyle::Both;
            o.size = 1;
        });
        e.color = Rgba([255, 0, 0, 255]);
        e.secondary = Rgba([0, 0, 255, 255]);
        let layer = e.target().unwrap();
        e.pointer_down((10, 10));
        e.pointer_move((20, 20));
        e.pointer_up();
        e.run_pending();
        assert_eq!(
            (pixel(&e, layer, 10, 10), pixel(&e, layer, 15, 15)),
            (Rgba([255, 0, 0, 255]), Rgba([0, 0, 255, 255]))
        );
        e.options.style = ShapeStyle::Filled;
        e.pointer_down((30, 10));
        e.pointer_move((40, 20));
        e.pointer_up();
        e.run_pending();
        assert_eq!(pixel(&e, layer, 35, 15), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn selecting_restricts_drawing_and_deselecting_restores_it() {
        let mut e = with_tool(Tool::Select, |_| {});
        let layer = e.target().unwrap();
        e.pointer_down((50, 50));
        e.pointer_move((10, 10));
        assert!(matches!(e.preview(), Some(Preview::Marquee { .. })));
        e.pointer_up();
        assert_eq!(e.run_pending(), 1);
        assert_eq!(
            e.selection(),
            Selection::Rect(paintcraft_core::types::Rect::new(10, 10, 41, 41).unwrap())
        );
        e.tool = Tool::Fill;
        e.pointer_down((200, 200));
        e.pointer_up();
        assert_eq!(
            e.run_pending(),
            0,
            "a seed outside the selection fills nothing"
        );
        e.pointer_down((20, 20));
        e.pointer_up();
        e.run_pending();
        assert_eq!(
            (
                pixel(&e, layer, 20, 20).a(),
                pixel(&e, layer, 9, 20).a(),
                pixel(&e, layer, 51, 20).a()
            ),
            (255, 0, 0)
        );
        e.request(91, Action::Select { corners: None });
        e.run_pending();
        assert_eq!(e.selection(), Selection::Inactive);
    }

    #[test]
    fn selecting_needs_no_target_layer_but_painting_does() {
        // A document with no layers at all.
        let mut e = Editor::with_document(
            paintcraft_document::new_document(32, 32, None, 0, POLICY).unwrap(),
            None,
        );
        e.tool = Tool::Select;
        e.pointer_down((1, 1));
        e.pointer_move((5, 5));
        e.pointer_up();
        assert_eq!(e.run_pending(), 1);
        e.tool = Tool::Pencil;
        e.pointer_down((1, 1));
        assert!(!e.gesture_in_progress() && e.status().contains("No editing target"));
    }

    #[test]
    fn autosave_checkpoints_unsaved_work_and_a_crash_can_be_recovered() {
        let dir = std::env::temp_dir().join(format!(
            "paintcraft-autosave-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let mut e = editor();
        e.attach_recovery(Recovery::start(&dir).unwrap());
        assert!(e.autosave(), "adding the layer left unsaved work");
        assert!(!e.autosave(), "nothing newer to write");
        let layer = e.target().unwrap();
        e.pointer_down((3, 3));
        assert!(!e.autosave(), "never mid-gesture");
        e.pointer_up();
        e.run_pending();
        assert!(e.autosave());
        let want = e.document().clone();
        // The process dies: the session lock is released but the checkpoint stays.
        drop(e);
        let found = Recovery::scan(&dir);
        assert_eq!(found.len(), 1);
        let back = Editor::recover(&found[0].checkpoint).unwrap();
        assert!(back.document().same_semantics(&want));
        assert_eq!(back.document().layers[&layer].raster.get(3, 3).a(), 255);
        assert!(back.is_dirty(), "recovered work is unsaved");
        Recovery::discard(&dir, found[0].session);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_confirmed_save_removes_the_checkpoint_and_clean_exit_leaves_nothing() {
        let dir = std::env::temp_dir().join(format!(
            "paintcraft-autosave2-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let file = dir.join("doc.pcnd");
        let mut e = editor();
        e.attach_recovery(Recovery::start(&dir.join("rec")).unwrap());
        e.set_path(file);
        assert!(e.autosave());
        e.request(500, Action::Save);
        e.run_pending();
        assert!(!e.is_dirty());
        drop(e);
        assert!(Recovery::scan(&dir.join("rec")).is_empty());
        assert_eq!(
            std::fs::read_dir(dir.join("rec")).unwrap().count(),
            0,
            "no stale files"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn trimming_history_is_reported_to_the_user() {
        let mut e = editor();
        e.tool = Tool::Brush;
        e.options.size = 1;
        e.set_history_budget(3 * paintcraft_raster::RasterLayer::TILE_BYTES as u64);
        let mut said = false;
        for i in 0..30 {
            stroke(&mut e, &[(10 + i, 10), (10 + i, 11)]);
            said |= e.status().contains("oldest undo step");
        }
        assert!(said, "the status must mention dropped history");
        assert!(e.undo_depth() >= 1 && e.undo_depth() < 31);
    }

    // ---- text -----------------------------------------------------------------------

    fn text_editor() -> Editor {
        let mut e = editor();
        e.select_tool(Tool::Text);
        e.options.text_scale = 1;
        e
    }

    fn lit(e: &Editor, layer: Uuid) -> usize {
        (0..320)
            .flat_map(|y| (0..320).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(e, layer, x, y).a() == 255)
            .count()
    }

    #[test]
    fn typing_then_clicking_elsewhere_places_the_text_as_one_transaction() {
        let mut e = text_editor();
        let layer = e.target().unwrap();
        let before = e.publications();
        e.pointer_down((10, 10));
        e.pointer_up();
        e.type_text("Hi");
        assert!(e.is_editing_text());
        assert!(matches!(e.preview(), Some(Preview::Text { ref text, .. }) if text == "Hi"));
        assert_eq!(e.run_pending(), 0, "typing publishes nothing");
        assert_eq!(lit(&e, layer), 0);
        e.pointer_down((100, 100)); // click elsewhere: places "Hi", starts a new text box
        e.pointer_up();
        assert_eq!(e.run_pending(), 1);
        assert_eq!(e.publications(), before + 1);
        assert!(lit(&e, layer) > 20);
        assert!(
            e.is_editing_text() && e.preview().is_some(),
            "a new empty text box is open"
        );
        e.request(60, Action::Undo);
        e.run_pending();
        assert_eq!(lit(&e, layer), 0, "the whole text is one undo step");
    }

    #[test]
    fn backspace_cancel_and_empty_text_place_nothing() {
        let mut e = text_editor();
        let layer = e.target().unwrap();
        e.pointer_down((5, 5));
        e.type_text("ab");
        e.backspace();
        e.backspace();
        e.backspace();
        e.commit_text();
        assert_eq!(e.run_pending(), 0, "empty text");
        e.pointer_down((5, 5));
        e.type_text("zzz");
        e.cancel_text();
        e.commit_text();
        assert_eq!((e.run_pending(), lit(&e, layer)), (0, 0));
        assert!(!e.is_editing_text());
    }

    #[test]
    fn switching_tools_saving_and_undoing_place_pending_text_first() {
        let mut e = text_editor();
        let layer = e.target().unwrap();
        e.pointer_down((5, 5));
        e.type_text("A");
        e.select_tool(Tool::Pencil);
        assert_eq!(e.run_pending(), 1);
        assert!(lit(&e, layer) > 0);
        e.select_tool(Tool::Text);
        e.pointer_down((50, 50));
        e.type_text("B");
        e.request(61, Action::Undo); // places B, then undoes it
        assert_eq!(e.run_pending(), 2);
        assert_eq!(pixel(&e, layer, 50, 52).a(), 0, "B was placed then undone");
        assert!(
            pixel(&e, layer, 5, 7).a() > 0 || lit(&e, layer) > 0,
            "A remains"
        );
    }

    #[test]
    fn characters_without_glyphs_are_skipped_and_reported() {
        let mut e = text_editor();
        e.pointer_down((5, 5));
        e.type_text("a\u{1F600}b\t漢");
        assert!(e.status().contains("Skipped 3"), "{}", e.status());
        e.type_text("\nc");
        let Some(Preview::Text { text, .. }) = e.preview() else {
            panic!()
        };
        assert_eq!(text, "ab\nc", "newline is kept");
        e.type_text("x");
        e.cancel_text();
        e.type_text("ignored when not editing");
        assert!(!e.is_editing_text());
    }

    #[test]
    fn text_needs_a_visible_target_like_any_painting() {
        let mut e = text_editor();
        let layer = e.target().unwrap();
        e.request(
            62,
            Action::SetVisible {
                layer,
                visible: false,
            },
        );
        e.run_pending();
        e.pointer_down((5, 5));
        assert!(!e.is_editing_text() && e.status().contains("hidden"));
    }

    #[test]
    fn layer_management_actions_are_single_undoable_transactions() {
        let mut e = editor();
        let bottom = e.document().roots[0];
        let top = e.target().unwrap();
        assert_eq!(e.layer_properties(top).map(|p| p.2), Some(1));
        let mut n = 100;
        let mut act = |e: &mut Editor, a: Action| {
            n += 1;
            e.request(n, a);
            e.run_pending()
        };
        assert_eq!(
            act(
                &mut e,
                Action::MoveLayer {
                    layer: top,
                    to_index: 0
                }
            ),
            1
        );
        assert_eq!(e.document().roots, [top, bottom]);
        assert_eq!(
            act(
                &mut e,
                Action::MoveLayer {
                    layer: top,
                    to_index: 0
                }
            ),
            0,
            "already there"
        );
        assert_eq!(
            act(
                &mut e,
                Action::Rename {
                    layer: top,
                    name: "Sky".into()
                }
            ),
            1
        );
        assert_eq!(
            act(
                &mut e,
                Action::Rename {
                    layer: top,
                    name: "Sky".into()
                }
            ),
            0,
            "same name changes nothing"
        );
        assert_eq!(
            act(
                &mut e,
                Action::SetOpacity {
                    layer: top,
                    opacity: 128
                }
            ),
            1
        );
        assert_eq!(e.layer_properties(top), Some(("Sky".into(), 128, 0)));
        // Deleting the target moves the target to a layer that exists; undo brings the layer back intact.
        assert_eq!(act(&mut e, Action::DeleteLayer { layer: top }), 1);
        assert_eq!(e.target(), Some(bottom));
        assert!(e.layer_properties(top).is_none());
        assert_eq!(act(&mut e, Action::Undo), 1);
        assert_eq!(e.layer_properties(top), Some(("Sky".into(), 128, 0)));
        // Acting on a layer that no longer exists fails cleanly.
        act(&mut e, Action::DeleteLayer { layer: bottom });
        let epoch = e.publications();
        assert_eq!(
            act(
                &mut e,
                Action::Rename {
                    layer: bottom,
                    name: "x".into()
                }
            ),
            0
        );
        assert_eq!(e.publications(), epoch);
        assert!(e.status().contains("failed"), "{}", e.status());
    }

    #[test]
    fn opening_a_bad_file_fails_without_a_panic() {
        assert!(Editor::open(Path::new("/definitely/not/here.pcnd")).is_err());
    }
}
