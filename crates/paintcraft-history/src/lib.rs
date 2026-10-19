//! Recorded history restores snapshots without re-executing edits.
use paintcraft_document::Document;
use paintcraft_raster::{RasterLayer, Selection};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

pub const DEFAULT_HISTORY_BYTES: u64 = 1 << 30;

/// Semantic presentation key; labels are metadata, never replay instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Initial,
    Edit,
    Batch,
    AddLayer,
    DeleteLayer,
    LayerProperties,
    ReorderLayer,
    Fill,
    Pencil,
    Brush,
    Erase,
    Selection,
    Shape,
    Text,
    ResizeCanvas,
}

impl Label {
    pub fn text(self) -> &'static str {
        match self {
            Self::Initial => "Initial state",
            Self::Edit => "Edit",
            Self::Batch => "Batch edit",
            Self::AddLayer => "Add layer",
            Self::DeleteLayer => "Delete layer",
            Self::LayerProperties => "Layer properties",
            Self::ReorderLayer => "Reorder layer",
            Self::Fill => "Fill",
            Self::Pencil => "Pencil",
            Self::Brush => "Brush",
            Self::Erase => "Erase",
            Self::Selection => "Selection",
            Self::Shape => "Shape",
            Self::Text => "Text",
            Self::ResizeCanvas => "Resize canvas",
        }
    }
}

#[derive(Debug)]
struct Entry {
    snapshot: Snapshot,
    label: Label,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryInfo {
    pub history_id: Option<Uuid>,
    pub state_id: Uuid,
    pub label: Label,
    pub current: bool,
    pub redo: bool,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub document: Arc<Document>,
    pub selection: Selection,
    pub history_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct History {
    entries: Vec<Entry>,
    cursor: usize,
    budget: u64,
    trimmed_total: u64,
    last_trimmed: usize,
}
impl History {
    pub fn new(initial: Snapshot) -> Self {
        Self {
            entries: vec![Entry {
                snapshot: initial,
                label: Label::Initial,
            }],
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
            for l in e.snapshot.document.layers.values() {
                seen.extend(l.raster.tile_identities());
            }
        }
        seen.len() as u64 * RasterLayer::TILE_BYTES as u64
    }
    pub fn undo_depth(&self) -> usize {
        self.cursor
    }
    pub fn redo_depth(&self) -> usize {
        self.entries.len() - self.cursor - 1
    }
    pub fn last_trimmed(&self) -> usize {
        self.last_trimmed
    }
    pub fn trimmed_total(&self) -> u64 {
        self.trimmed_total
    }
    pub fn first(&self) -> &Snapshot {
        &self.entries[0].snapshot
    }
    pub fn record(&mut self, snapshot: Snapshot) {
        self.record_labeled(snapshot, Label::Edit);
    }
    pub fn record_labeled(&mut self, snapshot: Snapshot, label: Label) {
        self.entries.truncate(self.cursor + 1);
        self.entries.push(Entry { snapshot, label });
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
        Some(self.entries[self.cursor].snapshot.clone())
    }
    pub fn redo(&mut self) -> Option<Snapshot> {
        if self.cursor + 1 >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        Some(self.entries[self.cursor].snapshot.clone())
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = EntryInfo> + '_ {
        self.entries.iter().enumerate().map(|(i, e)| EntryInfo {
            history_id: e.snapshot.history_id,
            state_id: e.snapshot.document.state_id,
            label: e.label,
            current: i == self.cursor,
            redo: i > self.cursor,
        })
    }

    /// Resolve a retained stable history identity; `bool` distinguishes a no-op.
    /// Missing/trimmed identities leave the cursor unchanged.
    pub fn seek(&mut self, id: Option<Uuid>) -> Option<(Snapshot, bool)> {
        let index = self
            .entries
            .iter()
            .position(|e| e.snapshot.history_id == id)?;
        let changed = index != self.cursor;
        self.cursor = index;
        Some((self.entries[index].snapshot.clone(), changed))
    }
}
