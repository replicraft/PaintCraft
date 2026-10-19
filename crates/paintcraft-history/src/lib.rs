//! Recorded history restores snapshots without re-executing edits.
use paintcraft_document::Document;
use paintcraft_raster::{RasterLayer, Selection};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

pub const DEFAULT_HISTORY_BYTES: u64 = 1 << 30;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub document: Arc<Document>,
    pub selection: Selection,
    pub history_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct History {
    entries: Vec<Snapshot>,
    cursor: usize,
    budget: u64,
    trimmed_total: u64,
    last_trimmed: usize,
}
impl History {
    pub fn new(initial: Snapshot) -> Self {
        Self {
            entries: vec![initial],
            cursor: 0,
            budget: DEFAULT_HISTORY_BYTES,
            trimmed_total: 0,
            last_trimmed: 0,
        }
    }
    pub fn set_budget(&mut self, bytes: u64) {
        self.budget = bytes;
    }
    pub fn resident_bytes(&self) -> u64 {
        let mut seen = HashSet::new();
        for e in &self.entries {
            for l in e.document.layers.values() {
                seen.extend(l.raster.tile_identities());
            }
        }
        seen.len() as u64 * RasterLayer::TILE_BYTES as u64
    }
    pub fn undo_depth(&self) -> usize {
        self.cursor
    }
    pub fn last_trimmed(&self) -> usize {
        self.last_trimmed
    }
    pub fn trimmed_total(&self) -> u64 {
        self.trimmed_total
    }
    pub fn first(&self) -> &Snapshot {
        &self.entries[0]
    }
    pub fn record(&mut self, snapshot: Snapshot) {
        self.entries.truncate(self.cursor + 1);
        self.entries.push(snapshot);
        self.cursor += 1;
        self.last_trimmed = 0;
        while self.cursor > 1 && self.resident_bytes() > self.budget {
            self.entries.remove(0);
            self.cursor -= 1;
            self.last_trimmed += 1;
        }
        self.trimmed_total += self.last_trimmed as u64;
    }
    pub fn undo(&mut self) -> Option<Snapshot> {
        self.cursor = self.cursor.checked_sub(1)?;
        Some(self.entries[self.cursor].clone())
    }
    pub fn redo(&mut self) -> Option<Snapshot> {
        if self.cursor + 1 >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        Some(self.entries[self.cursor].clone())
    }
}
