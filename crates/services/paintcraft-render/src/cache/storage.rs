use super::*;
use std::mem::size_of;

#[derive(Debug)]
struct Entry {
    key: Key,
    pixels: Vec<u8>,
    touched: u64,
    prefix: bool,
}
impl Entry {
    fn weight(&self) -> usize {
        self.key.weight() + self.pixels.capacity()
    }
}
#[derive(Debug)]
pub(super) struct Partition {
    entries: Vec<Entry>,
    budget: usize,
    clock: u64,
}
impl Partition {
    pub fn new(budget: usize, max_entries: usize) -> Result<Self> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(max_entries.min(budget / size_of::<Entry>()))
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "cannot allocate cache entry table",
                )
            })?;
        if entries.capacity() * size_of::<Entry>() > budget {
            entries = Vec::new();
        }
        Ok(Self {
            entries,
            budget,
            clock: 0,
        })
    }
    pub fn usage(&self) -> CacheUsage {
        CacheUsage {
            pixel_bytes: self.entries.iter().map(|e| e.pixels.capacity()).sum(),
            metadata_charge: self.entries.capacity() * size_of::<Entry>()
                + self.entries.iter().map(|e| e.key.weight()).sum::<usize>(),
            entries: self.entries.len(),
        }
    }
    pub fn key_budget(&self, pixels: usize) -> usize {
        self.budget
            .saturating_sub(self.entries.capacity() * size_of::<Entry>() + pixels)
    }
    pub fn find(&self, key: &Key) -> Option<usize> {
        self.entries.iter().position(|e| e.key == *key)
    }
    pub fn touch(&mut self, index: usize) -> &[u8] {
        self.clock = self.clock.wrapping_add(1);
        self.entries[index].touched = self.clock;
        &self.entries[index].pixels
    }
    pub fn insert(&mut self, key: Key, pixels: Vec<u8>, prefix: bool) {
        let entry = Entry {
            key,
            pixels,
            prefix,
            touched: self.clock.wrapping_add(1),
        };
        if self.entries.capacity() == 0
            || entry.weight()
                > self
                    .budget
                    .saturating_sub(self.entries.capacity() * size_of::<Entry>())
        {
            return;
        }
        // A coordinate has one current full version and one prefix per domain. Previously returned
        // output buffers are independent and cannot pin these cache Vecs.
        if let Some(i) = self
            .entries
            .iter()
            .position(|e| e.key.rect == entry.key.rect && e.prefix == prefix)
        {
            self.entries.swap_remove(i);
        }
        while self.entries.len() == self.entries.capacity()
            || self.usage().weight() + entry.weight() > self.budget
        {
            let Some(index) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.touched)
                .map(|(i, _)| i)
            else {
                return;
            };
            self.entries.swap_remove(index);
        }
        self.clock = entry.touched;
        self.entries.push(entry);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
