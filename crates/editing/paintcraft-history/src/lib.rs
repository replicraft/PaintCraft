//! Recorded history restores snapshots without re-executing edits.
use paintcraft_document::Document;
use paintcraft_raster::{RasterLayer, Selection};
use std::sync::Arc;
use uuid::Uuid;

pub const DEFAULT_HISTORY_BYTES: u64 = 1 << 30;

/// Semantic presentation key; labels are metadata, never replay instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Initial,
    Edit,
    Batch,
    AddLayer,
    DuplicateLayer,
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
    ClearCanvas,
}

impl Label {
    pub fn text(self) -> &'static str {
        match self {
            Self::Initial => "Initial state",
            Self::Edit => "Edit",
            Self::Batch => "Batch edit",
            Self::AddLayer => "Add layer",
            Self::DuplicateLayer => "Duplicate layer",
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
            Self::ClearCanvas => "Clear Canvas",
        }
    }
}

mod archive;
mod compression;
pub use compression::Compression;
mod memory;
mod pressure;
mod spill;
pub use memory::MemoryUsage;
use paintcraft_core::types::{Error, Result};
use paintcraft_raster::TileVersion;
pub use spill::{Retention, SpillReport};

#[derive(Clone, Debug)]
enum Storage {
    Hot(Snapshot),
    Cold(Arc<archive::Packed>),
}
#[derive(Clone, Debug)]
struct Entry {
    storage: Storage,
    label: Label,
}
impl Entry {
    fn state_id(&self) -> Uuid {
        match &self.storage {
            Storage::Hot(s) => s.document.state_id,
            Storage::Cold(s) => s.state_id,
        }
    }
    fn history_id(&self) -> Option<Uuid> {
        match &self.storage {
            Storage::Hot(s) => s.history_id,
            Storage::Cold(s) => s.history_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packing {
    Shared,
    Adaptive,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Compaction {
    pub packed_entries: usize,
    pub scanned_sources: usize,
    pub deferred_sources: usize,
    pub decoded_before: u64,
    pub decoded_after: u64,
    pub encoded_bytes: u64,
    pub codec_workspace_peak: u64,
    pub codec_scratch_bytes: u64,
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
    memo: archive::Memo,
    packing: Packing,
    retention: Retention,
    compression: Compression,
    spill_store: Option<Arc<dyn paintcraft_core::types::storage::SpillStore>>,
    last_cleanup_error: Option<Error>,
    restore_pixels: u64,
    last_packing_error: Option<Error>,
}
impl History {
    pub fn new(initial: Snapshot) -> Self {
        Self {
            entries: vec![Entry {
                storage: Storage::Hot(initial),
                label: Label::Initial,
            }],
            cursor: 0,
            budget: DEFAULT_HISTORY_BYTES,
            trimmed_total: 0,
            last_trimmed: 0,
            memo: archive::Memo::new(),
            packing: Packing::Adaptive,
            retention: Retention::Preserve,
            compression: Compression::None,
            spill_store: None,
            last_cleanup_error: None,
            restore_pixels: 268_435_456,
            last_packing_error: None,
        }
    }
    pub fn set_budget(&mut self, bytes: u64) {
        self.budget = bytes;
    }
    pub fn set_retention(&mut self, retention: Retention) {
        self.retention = retention;
    }
    pub fn set_packing(&mut self, packing: Packing) {
        self.packing = packing;
    }
    pub fn set_compression(&mut self, compression: Compression) {
        self.compression = compression;
    }
    pub fn set_restore_budget(&mut self, pixels: u64) {
        self.restore_pixels = pixels;
    }
    pub fn resident_bytes(&self) -> u64 {
        self.memory_usage().decoded_bytes
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
    pub fn first_state_id(&self) -> Uuid {
        self.entries[0].state_id()
    }
    pub fn last_packing_error(&self) -> Option<&Error> {
        self.last_packing_error.as_ref()
    }
    fn reclaim_tables(&mut self) {
        self.memo.retain(|key, _| key.resident_bytes() != 0);
        if self.memo.capacity() > 16 && self.memo.len().saturating_mul(4) < self.memo.capacity() {
            let mut small = archive::Memo::new();
            if small.try_reserve(self.memo.len()).is_ok() {
                small.extend(std::mem::take(&mut self.memo));
                self.memo = small;
            }
        }
        if self.entries.capacity() > 16
            && self.entries.len().saturating_mul(4) < self.entries.capacity()
        {
            let mut small = Vec::new();
            if small.try_reserve_exact(self.entries.len()).is_ok() {
                small.append(&mut self.entries);
                self.entries = small;
            }
        }
    }
    pub fn record(&mut self, snapshot: Snapshot) -> Result<()> {
        self.record_labeled(snapshot, Label::Edit)
    }
    pub fn record_labeled(&mut self, snapshot: Snapshot, label: Label) -> Result<()> {
        self.record_labeled_checked(snapshot, label, || Ok(()))
    }
    /// Cancellation and failed storage preparation leave this timeline intact.
    pub fn record_labeled_checked(
        &mut self,
        snapshot: Snapshot,
        label: Label,
        check: impl Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        let mut staged = self.try_fork()?;
        staged.record_in_place(snapshot, label, &check)?;
        check()?;
        *self = staged;
        Ok(())
    }
    fn record_in_place(
        &mut self,
        snapshot: Snapshot,
        label: Label,
        check: &impl Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        let previous_snapshot = self.restore(self.cursor, check)?;
        self.entries.try_reserve(1).map_err(|_| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "cannot reserve recorded history entry",
            )
        })?;
        let docs =
            std::iter::once(&previous_snapshot.document).chain(std::iter::once(&snapshot.document));
        let count = docs
            .clone()
            .flat_map(|doc| doc.layers.values())
            .map(|l| l.raster.resident_tiles())
            .sum();
        self.memo.try_reserve(count).map_err(|_| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "cannot track history decoded versions",
            )
        })?;
        for doc in docs {
            for layer in doc.layers.values() {
                for (_, lease) in layer.raster.tile_leases() {
                    self.memo.entry(lease.version()).or_default();
                }
            }
        }
        self.entries[self.cursor].storage = Storage::Hot(previous_snapshot);
        self.entries.truncate(self.cursor + 1);
        self.entries.push(Entry {
            storage: Storage::Hot(snapshot),
            label,
        });
        self.cursor += 1;
        self.reclaim_tables();
        self.last_trimmed = 0;
        self.last_packing_error = None;
        if self.retention == Retention::Preserve {
            self.relieve_pressure(check)?;
        } else if self.packing == Packing::Adaptive
            && self.budget != 0
            && self.memory_usage().retained_weight() > self.budget
        {
            let temporary = self.budget.min(64 << 20) as usize;
            if let Err(error) = self.compact_limited(2, temporary, 64, 64, check) {
                self.last_packing_error = Some(error);
            }
        }
        while self.retention == Retention::TrimOldest
            && self.cursor > 1
            && self.memory_usage().retained_weight() > self.budget
        {
            self.entries.remove(0);
            self.cursor -= 1;
            self.last_trimmed += 1;
        }
        // Weak metadata tracks readers even after logical branch/retention removal.
        self.reclaim_tables();
        self.trimmed_total += self.last_trimmed as u64;
        Ok(())
    }
    /// Prepare all cold records before replacing any entry. Cancellation/admission
    /// failure leaves the recorded timeline, cursor and published document intact.
    pub fn compact(
        &mut self,
        keep_hot: usize,
        temporary_bytes: usize,
        check: impl Fn() -> Result<()>,
    ) -> Result<Compaction> {
        self.compact_limited(keep_hot, temporary_bytes, usize::MAX, usize::MAX, check)
    }
    // Automatic pressure work scans at most 64 new source tiles and prepares
    // at most 64 complete cold entries per preparation. Other sources register
    // immutable raw leases without scanning their pixels.
    fn compact_limited(
        &mut self,
        keep_hot: usize,
        temporary_bytes: usize,
        max_entries: usize,
        max_sources: usize,
        check: impl Fn() -> Result<()>,
    ) -> Result<Compaction> {
        check()?;
        let before = self.memory_usage().decoded_bytes;
        let mut budget = temporary_bytes;
        let mut memo = archive::clone_memo(&self.memo, &mut budget)?;
        let entry_bytes = self.entries.len() * std::mem::size_of::<Entry>();
        if entry_bytes > budget {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "history entry admission exceeded",
            ));
        }
        budget -= entry_bytes;
        let mut entries = Vec::new();
        entries.try_reserve_exact(self.entries.len()).map_err(|_| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "cannot stage history entries",
            )
        })?;
        entries.extend(self.entries.iter().cloned());
        let end = entries.len().saturating_sub(keep_hot.max(1));
        let mut encoder = compression::Encoder::new(self.compression);
        let mut count = 0;
        let mut visited = 0;
        let mut sources_left = max_sources;
        let mut deferred = 0;
        for (i, entry) in entries.iter_mut().enumerate().take(end) {
            check()?;
            if i == self.cursor {
                continue;
            }
            if let Storage::Hot(snapshot) = &entry.storage {
                if visited >= max_entries {
                    break;
                }
                visited += 1;
                let packed = archive::Packed::build(
                    snapshot,
                    &mut memo,
                    &mut budget,
                    &check,
                    &mut encoder,
                    &mut sources_left,
                    &mut deferred,
                )?;
                entry.storage = Storage::Cold(Arc::new(packed));
                count += 1;
            }
        }
        check()?;
        self.entries = entries;
        self.memo = memo;
        let usage = self.memory_usage();
        Ok(Compaction {
            packed_entries: count,
            scanned_sources: max_sources - sources_left,
            deferred_sources: deferred,
            decoded_before: before,
            decoded_after: usage.decoded_bytes,
            encoded_bytes: usage.encoded_bytes,
            codec_workspace_peak: encoder.workspace_peak as u64,
            codec_scratch_bytes: encoder.scratch_bytes() as u64,
        })
    }
    fn restore(&mut self, index: usize, check: &impl Fn() -> Result<()>) -> Result<Snapshot> {
        check()?;
        let result = (|| {
            let snapshot = match &self.entries[index].storage {
                Storage::Hot(snapshot) => Ok(snapshot.clone()),
                Storage::Cold(packed) => packed.restore(self.restore_pixels, &mut self.memo, check),
            }?;
            check()?;
            Ok(snapshot)
        })();
        if result.is_err() {
            // A later tile can fail after earlier private decodes registered
            // aliases. Their leases have now dropped: retries must not retain
            // one dead identity per successful partial decode.
            self.reclaim_tables();
        }
        result
    }
    pub fn undo(&mut self) -> Result<Option<Snapshot>> {
        self.undo_checked(|| Ok(()))
    }
    pub fn undo_checked(&mut self, check: impl Fn() -> Result<()>) -> Result<Option<Snapshot>> {
        check()?;
        let Some(index) = self.cursor.checked_sub(1) else {
            return Ok(None);
        };
        let snapshot = self.restore(index, &check)?;
        self.cursor = index;
        self.reclaim_tables();
        Ok(Some(snapshot))
    }
    pub fn redo(&mut self) -> Result<Option<Snapshot>> {
        self.redo_checked(|| Ok(()))
    }
    pub fn redo_checked(&mut self, check: impl Fn() -> Result<()>) -> Result<Option<Snapshot>> {
        check()?;
        let index = self.cursor + 1;
        if index >= self.entries.len() {
            return Ok(None);
        }
        let snapshot = self.restore(index, &check)?;
        self.cursor = index;
        self.reclaim_tables();
        Ok(Some(snapshot))
    }
    pub fn entries(&self) -> impl ExactSizeIterator<Item = EntryInfo> + '_ {
        self.entries.iter().enumerate().map(|(i, e)| EntryInfo {
            history_id: e.history_id(),
            state_id: e.state_id(),
            label: e.label,
            current: i == self.cursor,
            redo: i > self.cursor,
        })
    }
    pub fn seek(&mut self, id: Option<Uuid>) -> Result<Option<(Snapshot, bool)>> {
        self.seek_checked(id, || Ok(()))
    }
    pub fn seek_checked(
        &mut self,
        id: Option<Uuid>,
        check: impl Fn() -> Result<()>,
    ) -> Result<Option<(Snapshot, bool)>> {
        check()?;
        let Some(index) = self.entries.iter().position(|e| e.history_id() == id) else {
            return Ok(None);
        };
        let snapshot = self.restore(index, &check)?;
        let changed = index != self.cursor;
        self.cursor = index;
        self.reclaim_tables();
        Ok(Some((snapshot, changed)))
    }
}

mod residency;
pub use residency::ResidencyMaintenance;
