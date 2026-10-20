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
    pub(super) fn fork_for_record(&self) -> Result<Self> {
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
        let result = self.prepare_spill(store.as_ref(), temporary_bytes, max_payloads, &check);
        // Failed preparation has dropped every unpublished extent by this point.
        self.last_cleanup_error = store.collect_garbage().err();
        result.map(|mut report| {
            report.store = store.usage();
            report
        })
    }
    fn prepare_spill(
        &mut self,
        store: &dyn SpillStore,
        temporary_bytes: usize,
        max_payloads: usize,
        check: &impl Fn() -> Result<()>,
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
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| archive::limit("cannot prepare history spill entries"))?;
        entries.extend(self.entries.iter().cloned());
        let mut replacements = std::collections::HashMap::new();
        let mut remaining = max_payloads;
        for entry in &mut entries {
            if let Storage::Cold(packed) = &entry.storage
                && let Some(packed) =
                    packed.spilled(&mut replacements, store, &mut budget, &mut remaining, check)?
            {
                entry.storage = Storage::Cold(Arc::new(packed));
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
        self.entries = entries;
        self.memo = memo;
        let usage = self.memory_usage();
        Ok(SpillReport {
            payloads: replacements.len(),
            decoded_before: before,
            decoded_after: usage.decoded_bytes,
            disk_bytes: usage.disk_bytes,
            store: store.usage(),
        })
    }
}
