use super::*;
use std::{collections::HashSet, mem::size_of};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryUsage {
    pub decoded_bytes: u64,
    /// Unique declared mask storage retained by hot or cold selection snapshots.
    /// Masks currently stay resident when raster history is packed or spilled.
    pub selection_bytes: u64,
    /// Known decoded allocations held beyond hot entries/raw payloads. This can
    /// include the active restored document and external render/save readers.
    pub leased_decoded_bytes: u64,
    pub encoded_bytes: u64,
    pub disk_bytes: u64,
    pub disk_payloads: usize,
    pub compressed_bytes: u64,
    pub compressed_payloads: usize,
    /// Conservative container/string/control-block weight, not allocator RSS.
    pub metadata_weight: u64,
    pub packed_entries: usize,
    pub logical_payloads: usize,
}
impl MemoryUsage {
    pub fn retained_weight(self) -> u64 {
        self.decoded_bytes - self.leased_decoded_bytes
            + self.selection_bytes
            + self.encoded_bytes
            + self.metadata_weight
    }
    pub fn known_weight(self) -> u64 {
        self.decoded_bytes + self.selection_bytes + self.encoded_bytes + self.metadata_weight
    }
}
impl History {
    pub fn memory_usage(&self) -> MemoryUsage {
        let mut owned: HashSet<TileVersion> = HashSet::new();
        let mut decoded: HashSet<TileVersion> = HashSet::new();
        let mut docs = HashSet::new();
        let mut blobs = HashSet::new();
        let mut masks = HashSet::new();
        let mut usage = MemoryUsage {
            metadata_weight: (self.entries.capacity() * size_of::<Entry>()
                + self.memo.capacity()
                    * (size_of::<(TileVersion, std::sync::Weak<archive::Blob>)>() + 160))
                as u64,
            ..Default::default()
        };
        for entry in &self.entries {
            let selection = match &entry.storage {
                Storage::Hot(snapshot) => &snapshot.selection,
                Storage::Cold(packed) => &packed.selection,
            };
            if let Selection::Mask(mask) = selection
                && let Some((identity, bytes)) = mask.storage_usage()
                && masks.insert(identity)
            {
                usage.selection_bytes += bytes;
            }
            match &entry.storage {
                Storage::Hot(s) => {
                    if docs.insert(Arc::as_ptr(&s.document)) {
                        usage.metadata_weight += archive::document_metadata(&s.document) as u64;
                    }
                    owned.extend(
                        s.document
                            .layers
                            .values()
                            .flat_map(|l| l.raster.tile_leases().map(|(_, t)| t.version())),
                    );
                }
                Storage::Cold(s) => {
                    usage.packed_entries += 1;
                    if let Some(doc) = s.decoded_document()
                        && docs.insert(Arc::as_ptr(&doc))
                    {
                        usage.metadata_weight += archive::document_metadata(&doc) as u64;
                    }
                    usage.metadata_weight += s.metadata_weight() as u64;
                    for blob in s.blobs() {
                        if blobs.insert(Arc::as_ptr(blob)) {
                            usage.logical_payloads += 1;
                            usage.encoded_bytes += blob.encoded_bytes() as u64;
                            let disk = blob.disk_bytes();
                            usage.disk_bytes += disk as u64;
                            usage.disk_payloads += usize::from(disk != 0);
                            let compressed = blob.compressed_bytes();
                            usage.compressed_bytes += compressed as u64;
                            usage.compressed_payloads += usize::from(compressed != 0);
                            usage.metadata_weight +=
                                (archive::blob_weight(blob) - blob.encoded_bytes()) as u64;
                            if blob.owns_decoded() {
                                owned.insert(blob.source());
                            }
                            decoded.insert(blob.source());
                        }
                    }
                }
            }
        }
        decoded.extend(self.memo.keys().cloned());
        usage.metadata_weight += owned.len() as u64 * 128;
        decoded.extend(owned.iter().cloned());
        for version in decoded {
            let bytes = version.resident_bytes() as u64;
            usage.decoded_bytes += bytes;
            if !owned.contains(&version) {
                usage.leased_decoded_bytes += bytes;
            }
        }
        usage
    }
}
