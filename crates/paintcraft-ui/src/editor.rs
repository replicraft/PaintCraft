//! The editor controller: one pencil, an explicit editing target, undo/redo and
//! safe save, all expressed as batch v1 transactions against a [`Session`].
//!
//! A gesture (pointer down, moves, up) is collected locally and committed as
//! exactly one transaction on release. Nothing is published while it is in
//! progress, so a cancelled or interrupted gesture changes nothing and a stroke
//! is one undo step. Commands reach the session only through the [`Outbox`].

use std::path::{Path, PathBuf};

use paintcraft_core::commands::Batch;
use paintcraft_core::document::{Document, POLICY, new_document};
use paintcraft_core::execute::Selection;
use paintcraft_core::session::Session;
use paintcraft_formats::{pcnd, save};
use paintcraft_types::json::Value;
use paintcraft_types::{Result, Rgba};
use uuid::Uuid;

use crate::outbox::{Outbox, Token};

/// Points per `raster.pencil_path` operation (the schema maximum).
const POINTS_PER_OP: usize = 65_536;
/// Operations per batch (the schema maximum); longer gestures are cut off.
const OPS_PER_BATCH: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Stroke {
        layer: Uuid,
        points: Vec<(i64, i64)>,
        color: Rgba,
    },
    AddLayer,
    SetVisible {
        layer: Uuid,
        visible: bool,
    },
    Undo,
    Redo,
    Save,
}

#[derive(Debug)]
struct Gesture {
    token: Token,
    layer: Uuid,
    color: Rgba,
    points: Vec<(i64, i64)>,
}

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
    pub color: Rgba,
    target: Option<Uuid>,
    gesture: Option<Gesture>,
    outbox: Outbox<Action>,
    next_gesture: Token,
    path: Option<PathBuf>,
    status: String,
}

/// Gesture tokens live in their own half of the token space so they can never
/// collide with the tokens the application derives for button activations.
const GESTURE_TOKEN_BASE: Token = 1 << 63;

impl Editor {
    /// A new white document with one `Background` layer as the editing target.
    pub fn new_document(width: u32, height: u32) -> Result<Editor> {
        let doc = new_document(width, height, Some(Rgba([255, 255, 255, 255])), 0, POLICY)?;
        let mut e = Editor::from_document(doc, None);
        e.status = format!("New {width}×{height} document");
        Ok(e)
    }

    pub fn open(path: &Path) -> Result<Editor> {
        let bytes = std::fs::read(path).map_err(|e| {
            paintcraft_types::Error::new(
                paintcraft_types::ErrorCode::StorageFailed,
                format!("cannot read file: {}", e.kind()),
            )
        })?;
        let mut e = Editor::from_document(pcnd::read(&bytes)?, Some(path.to_path_buf()));
        e.status = format!("Opened {}", path.display());
        Ok(e)
    }

    fn from_document(doc: Document, path: Option<PathBuf>) -> Editor {
        let target = doc.roots.last().copied();
        Editor {
            session: Session::new(doc),
            color: Rgba([0, 0, 0, 255]),
            target,
            gesture: None,
            outbox: Outbox::default(),
            next_gesture: GESTURE_TOKEN_BASE,
            path,
            status: String::new(),
        }
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
        self.gesture.is_some()
    }

    /// Points collected so far, for a live overlay.
    pub fn gesture_points(&self) -> &[(i64, i64)] {
        self.gesture.as_ref().map_or(&[], |g| &g.points)
    }

    // ---- pointer input ----------------------------------------------------------

    pub fn pointer_down(&mut self, doc_point: (i64, i64)) {
        if self.gesture.is_some() {
            return; // a gesture already owns the pointer
        }
        let Some(layer) = self
            .target
            .filter(|id| self.document().layers.contains_key(id))
        else {
            self.status = "No editing target: add or choose a layer".into();
            return;
        };
        if !self.document().layers[&layer].visible {
            self.status = "The target layer is hidden; show it or choose another layer".into();
            return;
        }
        self.next_gesture += 1;
        self.gesture = Some(Gesture {
            token: self.next_gesture,
            layer,
            color: self.color,
            points: vec![doc_point],
        });
    }

    pub fn pointer_move(&mut self, doc_point: (i64, i64)) {
        if let Some(g) = &mut self.gesture
            && g.points.last() != Some(&doc_point)
            && g.points.len() < POINTS_PER_OP * OPS_PER_BATCH
        {
            g.points.push(doc_point);
        }
    }

    /// Finish the gesture: queue exactly one stroke. A second call does nothing.
    pub fn pointer_up(&mut self) {
        if let Some(g) = self.gesture.take() {
            self.outbox.push(
                g.token,
                Action::Stroke {
                    layer: g.layer,
                    points: g.points,
                    color: g.color,
                },
            );
        }
    }

    /// Escape, focus loss, device removal or a modal dialog: drop the gesture unpublished.
    pub fn cancel_gesture(&mut self) {
        self.gesture = None;
    }

    // ---- commands ---------------------------------------------------------------

    /// Queue a command for one user activation. `token` must be stable across
    /// layout passes of the same activation and different between activations.
    pub fn request(&mut self, token: Token, action: Action) -> bool {
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
            } => self.stroke(layer, &points, color),
            Action::AddLayer => self.add_layer(),
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
    ) -> Option<paintcraft_core::session::Receipt> {
        let t = self.session.ticket();
        let batch = Value::object([
            ("schema", Value::str("paintcraft.batch/v1")),
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
                Some(r)
            }
            Err(e) => {
                self.status = format!("Edit failed: {e}");
                None
            }
        }
    }

    fn stroke(&mut self, layer: Uuid, points: &[(i64, i64)], color: Rgba) {
        if points.is_empty() {
            return;
        }
        // Long gestures are split across operations; each starts on the previous
        // chunk's last point so the path stays connected. A translucent colour
        // blends that one joint pixel twice.
        let mut ops = Vec::new();
        let mut start = 0;
        while start < points.len() && ops.len() < OPS_PER_BATCH {
            let end = (start + POINTS_PER_OP).min(points.len());
            let pts = points[start..end]
                .iter()
                .map(|(x, y)| Value::object([("x", Value::Int(*x)), ("y", Value::Int(*y))]))
                .collect();
            ops.push(Value::object([
                ("op", Value::str("raster.pencil_path")),
                (
                    "layer_ref",
                    Value::object([("id", Value::str(layer.to_string()))]),
                ),
                ("points", Value::Array(pts)),
                ("color", Value::str(color.to_hex())),
                ("mode", Value::str("source-over")),
            ]));
            if end == points.len() {
                break;
            }
            start = end - 1;
        }
        let drawn = self.commit(ops, "Pencil stroke");
        if drawn.as_ref().is_some_and(|r| !r.committed) {
            self.status = "Stroke changed nothing".into();
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

    fn save(&mut self) {
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
            self.session.mark_saved();
            self.status = format!("Saved {}", path.display());
        } else {
            // Dirty state stays: an unconfirmed or unknown save is not a clean document.
            self.status = format!("Not saved safely: {}", outcome.reasons.join("; "));
        }
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
        assert_ne!(
            e.document().state_id,
            before,
            "undo restored the post-AddLayer state, not the start"
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

    #[test]
    fn opening_a_bad_file_fails_without_a_panic() {
        assert!(Editor::open(Path::new("/definitely/not/here.pcnd")).is_err());
    }
}
