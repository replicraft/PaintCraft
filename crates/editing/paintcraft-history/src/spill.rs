use super::*;
use paintcraft_core::types::storage::{SpillStore, SpillUsage};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Retention {
    #[default]
    Preserve,
    TrimOldest,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpillReport {
    pub payloads: usize,
    pub decoded_before: u64,
    pub decoded_after: u64,
    pub disk_bytes: u64,
    pub store: SpillUsage,
}
impl History {
    /// Fallibly copy timeline metadata, sharing immutable documents and payloads.
    /// The 64 MiB metadata staging allowance does not reserve decoded pixels.
    /// Mutations of the returned history do not replace this timeline.
    pub fn try_fork(&self) -> Result<Self> {
        let mut budget = 64 << 20;
        let memo = archive::clone_memo(&self.memo, &mut budget)?;
        let count = self
            .entries
            .len()
            .checked_add(1)
            .ok_or_else(|| archive::limit("history entry count overflow"))?;
        let bytes = count
            .checked_mul(std::mem::size_of::<Entry>())
            .ok_or_else(|| archive::limit("history entry size overflow"))?;
        if bytes > budget {
            return Err(archive::limit("history publication staging exceeded"));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| archive::limit("cannot prepare reversible history"))?;
        entries.extend(self.entries.iter().cloned());
        Ok(Self {
            entries,
            cursor: self.cursor,
            budget: self.budget,
            trimmed_total: self.trimmed_total,
            last_trimmed: self.last_trimmed,
            memo,
            packing: self.packing,
            retention: self.retention,
            compression: self.compression,
            spill_store: self.spill_store.clone(),
            last_cleanup_error: self.last_cleanup_error.clone(),
            restore_pixels: self.restore_pixels,
            last_packing_error: self.last_packing_error.clone(),
        })
    }

    pub fn set_spill_store(&mut self, store: Arc<dyn SpillStore>) {
        self.spill_store = Some(store);
    }
    pub fn spill_usage(&self) -> Option<SpillUsage> {
        self.spill_store.as_ref().map(|s| s.usage())
    }
    pub fn last_cleanup_error(&self) -> Option<&Error> {
        self.last_cleanup_error.as_ref()
    }
    /// Write every replacement privately; existing records survive any failure.
    pub fn spill(
        &mut self,
        temporary_bytes: usize,
        max_payloads: usize,
        check: impl Fn() -> Result<()>,
    ) -> Result<SpillReport> {
        let store = self.spill_store.clone().ok_or_else(|| {
            Error::new(
                paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                "history has no spill store",
            )
        })?;
        let result = self.prepare_transfer(
            Some(store.as_ref()),
            temporary_bytes,
            max_payloads,
            &check,
            false,
        );
        // Failed preparation has dropped every unpublished extent by this point.
        self.last_cleanup_error = store.collect_garbage().err();
        result.map(|mut report| {
            report.store = store.usage();
            report
        })
    }
    pub(super) fn spill_unpinned(
        &mut self,
        temporary_bytes: usize,
        max_payloads: usize,
        check: &impl Fn() -> Result<()>,
    ) -> Result<SpillReport> {
        let store = self.spill_store.clone().expect("checked spill store");
        let result = self.prepare_transfer(
            Some(store.as_ref()),
            temporary_bytes,
            max_payloads,
            check,
            true,
        );
        self.last_cleanup_error = store.collect_garbage().err();
        result.map(|mut report| {
            report.store = store.usage();
            report
        })
    }
    pub(super) fn refine_deferred(
        &mut self,
        temporary_bytes: usize,
        max_payloads: usize,
        check: &impl Fn() -> Result<()>,
    ) -> Result<SpillReport> {
        self.prepare_transfer(None, temporary_bytes, max_payloads, check, true)
    }
    fn prepare_transfer(
        &mut self,
        store: Option<&dyn SpillStore>,
        temporary_bytes: usize,
        max_payloads: usize,
        check: &impl Fn() -> Result<()>,
        exclude_hot: bool,
    ) -> Result<SpillReport> {
        check()?;
        let before = self.memory_usage().decoded_bytes;
        let mut budget = temporary_bytes;
        let mut memo = archive::clone_memo(&self.memo, &mut budget)?;
        let charge = self.entries.len() * std::mem::size_of::<Entry>();
        if charge > budget {
            return Err(archive::limit("history spill entry admission exceeded"));
        }
        budget -= charge;
        let mut pinned = std::collections::HashSet::new();
        if exclude_hot {
            for entry in &self.entries {
                check()?;
                if let Storage::Hot(snapshot) = &entry.storage {
                    for layer in snapshot.document.layers.values() {
                        for (_, lease) in layer.raster.tile_leases() {
                            check()?;
                            let version = lease.version();
                            if !pinned.contains(&version) {
                                if budget < 128 {
                                    return Err(archive::limit(
                                        "history hot identity admission exceeded",
                                    ));
                                }
                                budget -= 128;
                                pinned.try_reserve(1).map_err(|_| {
                                    archive::limit("cannot track hot history tiles")
                                })?;
                                pinned.insert(version);
                            }
                        }
                    }
                }
            }
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| archive::limit("cannot prepare history spill entries"))?;
        entries.extend(self.entries.iter().cloned());
        let mut replacements = std::collections::HashMap::new();
        let mut remaining = max_payloads;
        let mut encoder = compression::Encoder::new(self.compression);
        {
            let mut work = archive::Transfer {
                replacements: &mut replacements,
                store,
                budget: &mut budget,
                remaining: &mut remaining,
                pinned: &pinned,
                encoder: &mut encoder,
            };
            for entry in &mut entries {
                if let Storage::Cold(packed) = &entry.storage
                    && let Some(packed) = packed.transferred(&mut work, check)?
                {
                    entry.storage = Storage::Cold(Arc::new(packed));
                }
            }
        }
        for target in memo.values_mut() {
            if let Some(old) = target.upgrade()
                && let Some(new) = replacements.get(&(Arc::as_ptr(&old) as usize))
            {
                *target = Arc::downgrade(new);
            }
        }
        check()?;
        if let Some(store) = store {
            store.flush()?;
        }
        check()?;
        self.entries = entries;
        self.memo = memo;
        let usage = self.memory_usage();
        Ok(SpillReport {
            payloads: replacements.len(),
            decoded_before: before,
            decoded_after: usage.decoded_bytes,
            disk_bytes: usage.disk_bytes,
            store: store.map(|s| s.usage()).unwrap_or_default(),
        })
    }
}
