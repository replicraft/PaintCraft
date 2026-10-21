//! One open document: the published state (document, selection, epoch) plus a
//! linear recorded history. A batch either publishes everything at once or
//! changes nothing. Undo and redo restore recorded snapshots; no algorithm runs.
//!
//! Snapshots share unchanged tiles by `Arc`, so a history entry costs only the
//! tiles an edit actually rewrote. History retention is bounded by its configured memory budget.

use std::collections::BTreeMap;
use std::sync::Arc;

use paintcraft_core::types::json::Value;
use paintcraft_core::types::{Error, ErrorCode, Result};
use uuid::Uuid;

use crate::execute::{STAGED_PIXELS_MAX, Selection, execute_with_admission_checked};
use paintcraft_commands::Batch;
use paintcraft_core::types::ids::deterministic_id;
use paintcraft_document::{Document, POLICY};
use paintcraft_raster::Admission;

pub use paintcraft_history::{
    Compression as HistoryCompression, DEFAULT_HISTORY_BYTES, Packing as HistoryPacking,
    Retention as HistoryRetention,
};
use paintcraft_history::{History, Snapshot};

mod history;
mod preparation;
mod residency;
pub use history::{HistoryTicket, HistoryView};
pub use preparation::{Preparation, PreparedEdit, PreparedNavigation};
pub use residency::PreparedHistoryMaintenance;

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
pub struct Session {
    owner: Uuid,
    doc: Arc<Document>,
    selection: Selection,
    epoch: u64,
    history: History,
    saved_state: Option<Uuid>,
    staged_pixels: u64,
    preparation_policy: Arc<()>,
}

impl Session {
    /// Open a document at publication epoch 0 with no history.
    pub fn new(doc: Document) -> Session {
        let doc = Arc::new(doc);
        Session {
            owner: Uuid::new_v4(),
            saved_state: Some(doc.state_id),
            history: History::new(Snapshot {
                document: Arc::clone(&doc),
                selection: Selection::Inactive,
                history_id: None,
            }),
            doc,
            selection: Selection::Inactive,
            epoch: 0,
            staged_pixels: STAGED_PIXELS_MAX,
            preparation_policy: Arc::new(()),
        }
    }

    /// Set history-owned retention weight. Preserving history packs/spills older
    /// states or rejects admission; only explicit TrimOldest drops entries.
    /// Reader leases are reported separately and cannot be forcibly freed.
    pub fn set_history_budget(&mut self, bytes: u64) {
        self.preparation_policy = Arc::new(());
        self.history.set_budget(bytes);
    }
    pub fn resident_bytes(&self) -> u64 {
        self.history.resident_bytes()
    }
    pub fn undo_depth(&self) -> usize {
        self.history.undo_depth()
    }
    pub fn redo_depth(&self) -> usize {
        self.history.redo_depth()
    }
    pub fn last_trimmed(&self) -> usize {
        self.history.last_trimmed()
    }
    pub fn trimmed_total(&self) -> u64 {
        self.history.trimmed_total()
    }

    pub fn history_memory(&self) -> paintcraft_history::MemoryUsage {
        self.history.memory_usage()
    }
    pub fn set_history_retention(&mut self, retention: HistoryRetention) {
        self.preparation_policy = Arc::new(());
        self.history.set_retention(retention);
    }
    pub fn history_spill_usage(&self) -> Option<paintcraft_core::types::storage::SpillUsage> {
        self.history.spill_usage()
    }
    pub fn set_history_spill_store(
        &mut self,
        store: Arc<dyn paintcraft_core::types::storage::SpillStore>,
    ) {
        self.preparation_policy = Arc::new(());
        self.history.set_spill_store(store);
    }
    pub fn spill_history(
        &mut self,
        temporary_bytes: usize,
        max_payloads: usize,
        check: impl Fn() -> Result<()>,
    ) -> Result<paintcraft_history::SpillReport> {
        self.preparation_policy = Arc::new(());
        self.history.spill(temporary_bytes, max_payloads, check)
    }
    pub fn set_history_packing(&mut self, packing: HistoryPacking) {
        self.preparation_policy = Arc::new(());
        self.history.set_packing(packing);
    }
    pub fn set_history_compression(&mut self, compression: HistoryCompression) {
        self.preparation_policy = Arc::new(());
        self.history.set_compression(compression);
    }
    pub fn set_history_restore_budget(&mut self, pixels: u64) {
        self.preparation_policy = Arc::new(());
        self.history.set_restore_budget(pixels);
    }
    pub fn compact_history(
        &mut self,
        keep_hot: usize,
        temporary_bytes: usize,
        check: impl Fn() -> Result<()>,
    ) -> Result<paintcraft_history::Compaction> {
        self.preparation_policy = Arc::new(());
        self.history.compact(keep_hot, temporary_bytes, check)
    }

    /// Lower the per-batch staging budget (resource-exhaustion testing, constrained hosts).
    pub fn set_staged_pixel_budget(&mut self, pixels: u64) {
        self.preparation_policy = Arc::new(());
        self.staged_pixels = pixels;
    }

    /// Freeze the published revision without cloning its tree or raster buffers.
    pub fn snapshot(&self) -> Arc<Document> {
        Arc::clone(&self.doc)
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
        Some(self.doc.state_id) != self.saved_state
    }

    /// The document has changes no file contains (for example, recovered work).
    pub fn mark_unsaved(&mut self) {
        self.saved_state = None;
    }

    pub fn mark_saved(&mut self) {
        self.mark_saved_state(self.doc.state_id);
    }

    /// A confirmed save acknowledges its captured state, even if editing advanced.
    pub fn mark_saved_state(&mut self, state: Uuid) {
        self.saved_state = Some(state);
    }

    /// Parse and execute a batch. Errors never change the session.
    pub fn apply_json(&mut self, bytes: &[u8]) -> Result<Receipt> {
        self.apply(&Batch::from_json(bytes)?)
    }

    pub fn apply(&mut self, batch: &Batch) -> Result<Receipt> {
        self.apply_checked(batch, || Ok(()))
    }

    fn apply_checked(&mut self, batch: &Batch, check: impl Fn() -> Result<()>) -> Result<Receipt> {
        self.apply_checked_with_admission(batch, Admission::new(self.staged_pixels), check)
    }
    fn apply_checked_with_admission(
        &mut self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<Receipt> {
        check()?;
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
        let mut staged = execute_with_admission_checked(
            &self.doc,
            self.selection,
            batch,
            admission.constrain_pixels(self.staged_pixels),
            &check,
        )?;
        check()?;
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
        let next_epoch = self.next_epoch()?;
        let base = self.doc.state_id.to_string();
        if content_changed {
            staged.doc.state_id = deterministic_id(&["state", &base, &batch.digest]);
        }
        let history_id =
            deterministic_id(&["history", &base, &self.epoch.to_string(), &batch.digest]);
        let doc = Arc::new(staged.doc);
        // The single publication point: document, selection, history and epoch together.
        self.history.record_labeled_checked(
            Snapshot {
                document: Arc::clone(&doc),
                selection: staged.selection,
                history_id: Some(history_id),
            },
            history::label(batch),
            check,
        )?;
        self.doc = doc;
        self.selection = staged.selection;
        self.epoch = next_epoch;
        Ok(Receipt {
            committed: true,
            state_id: self.doc.state_id,
            publication_epoch: self.epoch,
            history_id: Some(history_id),
            aliases: staged.aliases,
        })
    }

    pub fn undo(&mut self) -> Result<Option<Navigation>> {
        self.undo_checked(|| Ok(()))
    }
    fn undo_checked(&mut self, check: impl Fn() -> Result<()>) -> Result<Option<Navigation>> {
        self.undo_with_admission(Admission::new(u64::MAX), check)
    }
    fn undo_with_admission(
        &mut self,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<Option<Navigation>> {
        check()?;
        if self.history.undo_depth() == 0 {
            return Ok(None);
        }
        let next_epoch = self.next_epoch()?;
        let snapshot = self.history.undo_with_admission(admission, check)?;
        Ok(snapshot.map(|snapshot| self.navigate(snapshot, next_epoch)))
    }
    pub fn redo(&mut self) -> Result<Option<Navigation>> {
        self.redo_checked(|| Ok(()))
    }
    fn redo_checked(&mut self, check: impl Fn() -> Result<()>) -> Result<Option<Navigation>> {
        self.redo_with_admission(Admission::new(u64::MAX), check)
    }
    fn redo_with_admission(
        &mut self,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<Option<Navigation>> {
        check()?;
        if self.history.redo_depth() == 0 {
            return Ok(None);
        }
        let next_epoch = self.next_epoch()?;
        let snapshot = self.history.redo_with_admission(admission, check)?;
        Ok(snapshot.map(|snapshot| self.navigate(snapshot, next_epoch)))
    }
    pub(super) fn next_epoch(&self) -> Result<u64> {
        self.epoch
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "publication epoch exhausted"))
    }
    fn navigate(&mut self, snapshot: Snapshot, next_epoch: u64) -> Navigation {
        self.doc = snapshot.document;
        self.selection = snapshot.selection;
        self.epoch = next_epoch;
        Navigation {
            state_id: self.doc.state_id,
            publication_epoch: self.epoch,
            history_id: snapshot.history_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::Rgba;
    use paintcraft_document::new_document;

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
    fn saving_a_captured_state_does_not_mark_newer_edits_clean() {
        let mut s = session();
        s.apply_json(&batch(&s, &format!("[{ADD}]"))).unwrap();
        let saved = s.ticket().state_id;
        let layer = s.document().roots[1];
        s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 0, "#000000FF"))))
            .unwrap();
        s.mark_saved_state(saved);
        assert!(s.is_dirty());
        s.undo().unwrap().unwrap();
        assert!(!s.is_dirty());
        s.redo().unwrap().unwrap();
        assert!(s.is_dirty());
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
        let n = s.undo().unwrap().unwrap();
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
        assert!(s.redo().unwrap().is_some());
        s.undo().unwrap().unwrap();
        s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 1, "#0000FFFF"))))
            .unwrap();
        assert!(
            s.redo().unwrap().is_none(),
            "editing after undo cuts the redo branch"
        );
        // Walk all the way back; the root has no history id.
        while s.undo().unwrap().is_some() {}
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
            s.undo().unwrap().map(|n| n.state_id),
            Some(doc_before_root_state(&s)),
            "no history entry was added"
        );
        s.redo().unwrap();
        s.set_staged_pixel_budget(STAGED_PIXELS_MAX);
        assert!(
            s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 0, "#000000FF"))))
                .unwrap()
                .committed
        );
    }

    fn doc_before_root_state(s: &Session) -> Uuid {
        s.history.first_state_id()
    }

    #[test]
    fn a_memory_budget_drops_only_the_oldest_undo_steps_and_says_so() {
        // Ten tiles side by side; every edit rewrites a different tile.
        let mut s = Session::new(new_document(2560, 256, None, 0, POLICY).unwrap());
        let r = s.apply_json(&batch(&s, r##"[{"op":"layer.add","ref":"l","name":"L","width":2560,"height":256,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}]"##)).unwrap();
        let layer = r.aliases["l"];
        let tile = paintcraft_raster::RasterLayer::TILE_BYTES as u64;
        s.set_history_retention(HistoryRetention::TrimOldest);
        s.set_history_packing(paintcraft_history::Packing::Shared);
        s.set_history_budget(14 * tile);
        let mut states = vec![s.ticket().state_id];
        for i in 0..60i64 {
            let x = (i % 10) * 256 + 5;
            s.apply_json(&batch(&s, &format!(r##"[{{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":{x},"y":{},"width":1,"height":1}},"color":"#000000FF","mode":"replace"}}]"##, i / 10))).unwrap();
            states.push(s.ticket().state_id);
            assert!(
                s.resident_bytes() <= 14 * tile || s.undo_depth() <= 1,
                "edit {i}: {} bytes, depth {}",
                s.resident_bytes(),
                s.undo_depth()
            );
        }
        assert!(
            s.trimmed_total() > 0 && s.undo_depth() < 60,
            "history is bounded"
        );
        assert!(s.undo_depth() >= 1);
        // Everything still reachable is exact: walking back restores the recorded states in order.
        let depth = s.undo_depth();
        for k in 0..depth {
            let want = states[states.len() - 2 - k];
            assert_eq!(s.undo().unwrap().unwrap().state_id, want, "undo step {k}");
        }
        assert!(s.undo().unwrap().is_none(), "nothing older is retained");
        // The current document was never touched by trimming.
        while s.redo().unwrap().is_some() {}
        assert_eq!(s.ticket().state_id, *states.last().unwrap());
    }

    #[test]
    fn the_newest_edit_stays_undoable_whatever_the_budget() {
        let mut s = session();
        s.set_history_retention(HistoryRetention::TrimOldest);
        s.set_history_budget(0);
        s.apply_json(&batch(&s, &format!("[{ADD}]"))).unwrap();
        let layer = s.document().roots[1];
        s.apply_json(&batch(&s, &format!("[{}]", fill(layer, 0, "#000000FF"))))
            .unwrap();
        assert_eq!(s.undo_depth(), 1);
        assert!(s.undo().unwrap().is_some());
        assert_eq!(
            s.document().layers[&layer].raster.get(0, 0),
            Rgba::TRANSPARENT
        );
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
    #[test]
    fn exhausted_publication_epoch_never_changes_document_or_history_and_allows_noops() {
        let mut s = session();
        s.apply_json(&batch(&s, &format!("[{ADD}]"))).unwrap();
        s.epoch = u64::MAX;
        let before = s.ticket();
        let entries = s.history_view().entries;
        let doc = s.snapshot();
        assert_eq!(
            s.apply_json(&batch(&s, &format!("[{ADD}]")))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(s.undo().unwrap_err().code, ErrorCode::ResourceLimit);
        let view = s.history_view();
        assert_eq!(
            s.navigate_history(view.ticket, view.entries[0].history_id)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        let current = view
            .entries
            .iter()
            .find(|entry| entry.current)
            .unwrap()
            .history_id;
        assert!(s.navigate_history(view.ticket, current).unwrap().is_none());
        assert!(s.redo().unwrap().is_none());
        assert_eq!(s.ticket(), before);
        assert_eq!(s.history_view().entries, entries);
        assert!(Arc::ptr_eq(&s.snapshot(), &doc));
        s.epoch = 1;
        s.undo().unwrap().unwrap();
        s.epoch = u64::MAX;
        let before = s.ticket();
        let entries = s.history_view().entries;
        assert_eq!(s.redo().unwrap_err().code, ErrorCode::ResourceLimit);
        let background = s.document().roots[0];
        let receipt = s
            .apply_json(&batch(
                &s,
                &format!("[{}]", fill(background, 0, "#FFFFFFFF")),
            ))
            .unwrap();
        assert!(!receipt.committed);
        assert_eq!(s.ticket(), before);
        assert_eq!(s.history_view().entries, entries);
    }
}
