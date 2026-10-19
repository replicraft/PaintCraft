//! One open document: the published state (document, selection, epoch) plus a
//! linear recorded history. A batch either publishes everything at once or
//! changes nothing. Undo and redo restore recorded snapshots; no algorithm runs.
//!
//! Snapshots share unchanged tiles by `Arc`, so a history entry costs only the
//! tiles an edit actually rewrote. Bounded eviction is a later task.

use std::collections::BTreeMap;
use std::sync::Arc;

use paintcraft_types::json::Value;
use paintcraft_types::{Error, ErrorCode, Result};
use uuid::Uuid;

use crate::commands::Batch;
use crate::document::{Document, POLICY};
use crate::execute::{STAGED_PIXELS_MAX, Selection, execute_with_budget};
use crate::ids::deterministic_id;

/// `paintcraft.result/v1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub committed: bool,
    pub state_id: Uuid,
    pub publication_epoch: u64,
    pub history_id: Option<Uuid>,
    pub aliases: BTreeMap<String, Uuid>,
}

impl Receipt {
    pub fn to_value(&self) -> Value {
        Value::object([
            ("schema", Value::str("paintcraft.result/v1")),
            ("committed", Value::Bool(self.committed)),
            ("state_id", Value::str(self.state_id.to_string())),
            (
                "publication_epoch",
                Value::str(self.publication_epoch.to_string()),
            ),
            (
                "history_id",
                self.history_id
                    .map_or(Value::Null, |h| Value::str(h.to_string())),
            ),
            (
                "aliases",
                Value::Object(
                    self.aliases
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::str(v.to_string())))
                        .collect(),
                ),
            ),
            ("warnings", Value::Array(Vec::new())),
        ])
    }
}

/// The result of undo or redo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    pub state_id: Uuid,
    pub publication_epoch: u64,
    pub history_id: Option<Uuid>,
}

/// The optimistic-concurrency ticket a batch must carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub state_id: Uuid,
    pub publication_epoch: u64,
}

#[derive(Debug)]
struct Entry {
    doc: Arc<Document>,
    selection: Selection,
    history_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct Session {
    doc: Arc<Document>,
    selection: Selection,
    epoch: u64,
    entries: Vec<Entry>,
    cursor: usize,
    saved_state: Uuid,
    staged_pixels: u64,
}

impl Session {
    /// Open a document at publication epoch 0 with no history.
    pub fn new(doc: Document) -> Session {
        let doc = Arc::new(doc);
        Session {
            saved_state: doc.state_id,
            entries: vec![Entry {
                doc: Arc::clone(&doc),
                selection: Selection::Inactive,
                history_id: None,
            }],
            doc,
            selection: Selection::Inactive,
            epoch: 0,
            cursor: 0,
            staged_pixels: STAGED_PIXELS_MAX,
        }
    }

    /// Lower the per-batch staging budget (resource-exhaustion testing, constrained hosts).
    pub fn set_staged_pixel_budget(&mut self, pixels: u64) {
        self.staged_pixels = pixels;
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn ticket(&self) -> Ticket {
        Ticket {
            state_id: self.doc.state_id,
            publication_epoch: self.epoch,
        }
    }

    /// Native dirty state: the current content differs from the last saved content.
    pub fn is_dirty(&self) -> bool {
        self.doc.state_id != self.saved_state
    }

    pub fn mark_saved(&mut self) {
        self.saved_state = self.doc.state_id;
    }

    /// Parse and execute a batch. Errors never change the session.
    pub fn apply_json(&mut self, bytes: &[u8]) -> Result<Receipt> {
        self.apply(&Batch::from_json(bytes)?)
    }

    pub fn apply(&mut self, batch: &Batch) -> Result<Receipt> {
        if batch.policy != POLICY {
            return Err(Error::new(
                ErrorCode::PolicyUnknown,
                format!("policy {:?} is not implemented", batch.policy),
            ));
        }
        if (batch.base_state, batch.base_epoch) != (self.doc.state_id, self.epoch) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "The current publication differs from the requested base.",
            ));
        }
        let mut staged = execute_with_budget(&self.doc, self.selection, batch, self.staged_pixels)?;
        let content_changed = !staged.doc.same_semantics(&self.doc);
        if !content_changed && staged.selection == self.selection {
            return Ok(Receipt {
                committed: false,
                state_id: self.doc.state_id,
                publication_epoch: self.epoch,
                history_id: None,
                aliases: BTreeMap::new(),
            });
        }
        let base = self.doc.state_id.to_string();
        if content_changed {
            staged.doc.state_id = deterministic_id(&["state", &base, &batch.digest]);
        }
        let history_id =
            deterministic_id(&["history", &base, &self.epoch.to_string(), &batch.digest]);
        let doc = Arc::new(staged.doc);
        // The single publication point: document, selection, history and epoch together.
        self.entries.truncate(self.cursor + 1);
        self.entries.push(Entry {
            doc: Arc::clone(&doc),
            selection: staged.selection,
            history_id: Some(history_id),
        });
        self.cursor += 1;
        self.doc = doc;
        self.selection = staged.selection;
        self.epoch += 1;
        Ok(Receipt {
            committed: true,
            state_id: self.doc.state_id,
            publication_epoch: self.epoch,
            history_id: Some(history_id),
            aliases: staged.aliases,
        })
    }

    pub fn undo(&mut self) -> Option<Navigation> {
        self.cursor.checked_sub(1).map(|t| self.navigate(t))
    }

    pub fn redo(&mut self) -> Option<Navigation> {
        (self.cursor + 1 < self.entries.len()).then(|| self.navigate(self.cursor + 1))
    }

    fn navigate(&mut self, target: usize) -> Navigation {
        let e = &self.entries[target];
        self.doc = Arc::clone(&e.doc);
        self.selection = e.selection;
        self.cursor = target;
        self.epoch += 1;
        Navigation {
            state_id: self.doc.state_id,
            publication_epoch: self.epoch,
            history_id: e.history_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::new_document;
    use paintcraft_types::Rgba;

    fn batch(s: &Session, ops: &str) -> Vec<u8> {
        let t = s.ticket();
        format!(
            r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
            t.state_id, t.publication_epoch
        )
        .into_bytes()
    }

    const ADD: &str = r##"{"op":"layer.add","ref":"a","name":"A","width":2,"height":1,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}"##;

    fn session() -> Session {
        Session::new(new_document(2, 1, Some(Rgba([255, 255, 255, 255])), 0, POLICY).unwrap())
    }

    fn fill(id: Uuid, x: i64, c: &str) -> String {
        format!(
            r##"{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":{x},"y":0,"width":1,"height":1}},"color":"{c}","mode":"replace"}}"##
        )
    }

    #[test]
    fn commit_noop_and_selection_only_outcomes() {
        let mut s = session();
        let state = s.ticket().state_id;
        let r = s.apply_json(&batch(&s, &format!("[{ADD}]"))).unwrap();
        assert!(r.committed && r.history_id.is_some() && r.aliases.contains_key("a"));
        assert_ne!(r.state_id, state);
        assert_eq!(r.publication_epoch, 1);
        let t = s.ticket();
        let r = s
            .apply_json(&batch(&s, r#"[{"op":"selection.clear"}]"#))
            .unwrap();
        assert_eq!(
            (r.committed, r.history_id, r.publication_epoch, r.state_id),
            (false, None, 1, t.state_id)
        );
        assert_eq!(s.ticket(), t);
        let r = s
            .apply_json(&batch(
                &s,
                r#"[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}}]"#,
            ))
            .unwrap();
        assert!(r.committed && r.state_id == t.state_id && r.publication_epoch == 2);
    }

    #[test]
    fn failed_batches_change_nothing() {
        let mut s = session();
        let t = s.ticket();
        let bad = batch(
            &s,
            &format!("[{ADD},{{\"op\":\"layer.delete\",\"layer_ref\":{{\"ref\":\"zz\"}}}}]"),
        );
        let e = s.apply_json(&bad).unwrap_err();
        assert_eq!(
            (e.code, e.operation_index),
            (ErrorCode::ReferenceUnknownAlias, Some(1))
        );
        assert_eq!(s.ticket(), t);
        assert_eq!(s.document().layers.len(), 1);
    }

    #[test]
    fn undo_redo_restore_recorded_state_and_dirty_tracking() {
        let mut s = session();
        s.apply_json(&batch(&s, &format!("[{ADD}]"))).unwrap();
        s.mark_saved();
        let saved = s.ticket().state_id;
        let layer = s.document().roots[1];
        s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 0, "#000000FF"))))
            .unwrap();
        assert!(s.is_dirty());
        let n = s.undo().unwrap();
        assert_eq!((n.state_id, n.publication_epoch), (saved, 3));
        assert!(
            !s.is_dirty(),
            "undo to the saved state is clean even though the epoch is newer"
        );
        // ABA: a ticket from before the undo is stale although the StateId matches.
        let stale =
            Batch::from_json(&batch(&s, &format!("[{}]", fill(layer, 1, "#0000FFFF")))).unwrap();
        let mut stale = stale;
        stale.base_epoch = 1;
        assert_eq!(
            s.apply(&stale).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert!(s.redo().is_some());
        s.undo().unwrap();
        s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 1, "#0000FFFF"))))
            .unwrap();
        assert!(
            s.redo().is_none(),
            "editing after undo cuts the redo branch"
        );
        // Walk all the way back; the root has no history id.
        while s.undo().is_some() {}
        assert_eq!(s.document().layers.len(), 1);
    }

    #[test]
    fn resource_exhaustion_leaves_document_history_and_epoch_unchanged() {
        let mut s = session();
        s.apply_json(&batch(&s, &format!("[{ADD}]"))).unwrap();
        let (ticket, doc_before) = (s.ticket(), s.document().clone());
        s.set_staged_pixel_budget(0);
        let layer = s.document().roots[1];
        let e = s
            .apply_json(&batch(&s, &format!("[{}]", fill(layer, 0, "#000000FF"))))
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::ResourceLimit);
        assert_eq!(s.ticket(), ticket);
        assert!(s.document().same_semantics(&doc_before));
        assert_eq!(
            s.undo().map(|n| n.state_id),
            Some(doc_before_root_state(&s)),
            "no history entry was added"
        );
        s.redo();
        s.set_staged_pixel_budget(STAGED_PIXELS_MAX);
        assert!(
            s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 0, "#000000FF"))))
                .unwrap()
                .committed
        );
    }

    fn doc_before_root_state(s: &Session) -> Uuid {
        s.entries[0].doc.state_id
    }

    #[test]
    fn unknown_policy_and_stale_base_are_distinguished() {
        let mut s = session();
        let text = String::from_utf8(batch(&s, &format!("[{ADD}]"))).unwrap();
        let e = s
            .apply_json(text.replace("default-v1", "missing-v1").as_bytes())
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::PolicyUnknown);
        let e = s
            .apply_json(
                text.replace("\"publication_epoch\":\"0\"", "\"publication_epoch\":\"9\"")
                    .as_bytes(),
            )
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::ConflictStaleBase);
    }
}
