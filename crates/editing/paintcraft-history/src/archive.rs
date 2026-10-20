//! Recorded cold snapshots contain metadata and immutable payload references.
//! They never retain a strong decoded source merely to identify its version.
use crate::*;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
use paintcraft_document::Layer;
use paintcraft_raster::{Admission, TileCoord, TileVersion};
use std::{
    collections::HashMap,
    mem::size_of,
    sync::{Mutex, Weak},
};
pub(crate) mod payload;
pub(crate) use payload::Blob;

pub(crate) type Memo = HashMap<TileVersion, Weak<Blob>>;
pub(crate) fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
#[derive(Debug)]
pub(crate) struct Packed {
    document_id: Uuid,
    pub state_id: Uuid,
    canvas: (u32, u32),
    roots: Vec<Uuid>,
    layers: Vec<SavedLayer>,
    title: Option<String>,
    pub selection: Selection,
    pub history_id: Option<Uuid>,
    decoded_doc: Mutex<Weak<Document>>,
}
#[derive(Debug)]
struct SavedLayer {
    id: Uuid,
    name: String,
    origin: (i64, i64),
    visible: bool,
    opacity: u8,
    width: u32,
    height: u32,
    default: Rgba,
    tiles: Vec<(TileCoord, Arc<Blob>)>,
}
impl Packed {
    pub fn build(
        snapshot: &Snapshot,
        memo: &mut Memo,
        budget: &mut usize,
        check: &impl Fn() -> Result<()>,
        encoder: &mut compression::Encoder,
    ) -> Result<Self> {
        let doc = &snapshot.document;
        // Conservative metadata admission before copying variable-sized structures.
        let estimate = size_of::<Self>()
            + 32
            + doc.roots.len() * size_of::<Uuid>()
            + doc.title.as_ref().map_or(0, String::len)
            + doc
                .layers
                .values()
                .map(|l| {
                    size_of::<SavedLayer>()
                        + l.name.len()
                        + l.raster.resident_tiles() * size_of::<(TileCoord, Arc<Blob>)>()
                })
                .sum::<usize>();
        if estimate > *budget {
            return Err(limit("history metadata admission exceeded"));
        }
        *budget -= estimate;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(doc.layers.len())
            .map_err(|_| limit("cannot allocate recorded layers"))?;
        for layer in doc.layers.values() {
            check()?;
            let mut tiles = Vec::new();
            tiles
                .try_reserve_exact(layer.raster.resident_tiles())
                .map_err(|_| limit("cannot allocate recorded tile references"))?;
            for (coord, lease) in layer.raster.tile_leases() {
                check()?;
                let version = lease.version();
                let blob = if let Some(blob) = memo.get(&version).and_then(Weak::upgrade) {
                    blob
                } else {
                    let charge = size_of::<Blob>() + 32 + 128;
                    if charge > *budget {
                        return Err(limit("history payload metadata exceeded"));
                    }
                    *budget -= charge;
                    let blob = Arc::new(Blob::pack(
                        lease,
                        layer.raster.default_pixel(),
                        budget,
                        check,
                        encoder,
                    )?);
                    memo.try_reserve(1)
                        .map_err(|_| limit("cannot allocate history version index"))?;
                    memo.insert(version, Arc::downgrade(&blob));
                    blob
                };
                tiles.push((coord, blob));
            }
            layers.push(SavedLayer {
                id: layer.id,
                name: layer.name.clone(),
                origin: layer.origin,
                visible: layer.visible,
                opacity: layer.opacity,
                width: layer.raster.width(),
                height: layer.raster.height(),
                default: layer.raster.default_pixel(),
                tiles,
            });
        }
        Ok(Self {
            document_id: doc.document_id,
            state_id: doc.state_id,
            canvas: doc.canvas,
            roots: doc.roots.clone(),
            layers,
            title: doc.title.clone(),
            selection: snapshot.selection,
            history_id: snapshot.history_id,
            decoded_doc: Mutex::new(Arc::downgrade(&snapshot.document)),
        })
    }
    pub fn restore(&self, pixels: u64, memo: &mut Memo) -> Result<Snapshot> {
        if let Some(document) = self.decoded_document() {
            return Ok(Snapshot {
                document,
                selection: self.selection,
                history_id: self.history_id,
            });
        }
        memo.try_reserve(self.layers.iter().map(|layer| layer.tiles.len()).sum())
            .map_err(|_| limit("cannot track restored decoded versions"))?;
        let mut admission = Admission::new(pixels);
        let mut layers = HashMap::new();
        layers
            .try_reserve(self.layers.len())
            .map_err(|_| limit("cannot allocate restored layers"))?;
        for layer in &self.layers {
            let mut raster = RasterLayer::new(layer.width, layer.height, layer.default);
            for (coord, blob) in &layer.tiles {
                let lease = blob.restore(&mut admission)?;
                memo.insert(lease.version(), Arc::downgrade(blob));
                raster.install_tile(*coord, lease)?;
            }
            layers.insert(
                layer.id,
                Layer {
                    id: layer.id,
                    name: layer.name.clone(),
                    origin: layer.origin,
                    visible: layer.visible,
                    opacity: layer.opacity,
                    raster,
                },
            );
        }
        let document = Arc::new(Document {
            document_id: self.document_id,
            state_id: self.state_id,
            canvas: self.canvas,
            roots: self.roots.clone(),
            layers,
            title: self.title.clone(),
        });
        *self.decoded_doc.lock().unwrap_or_else(|p| p.into_inner()) = Arc::downgrade(&document);
        Ok(Snapshot {
            document,
            selection: self.selection,
            history_id: self.history_id,
        })
    }
    pub fn decoded_document(&self) -> Option<Arc<Document>> {
        self.decoded_doc
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .upgrade()
    }
    pub fn metadata_weight(&self) -> usize {
        size_of::<Self>()
            + size_of::<Document>()
            + 32
            + 32
            + self.roots.capacity() * size_of::<Uuid>()
            + self.layers.capacity() * size_of::<SavedLayer>()
            + self.title.as_ref().map_or(0, String::capacity)
            + self
                .layers
                .iter()
                .map(|l| {
                    l.name.capacity() + l.tiles.capacity() * size_of::<(TileCoord, Arc<Blob>)>()
                })
                .sum::<usize>()
    }
    pub fn blobs(&self) -> impl Iterator<Item = &Arc<Blob>> {
        self.layers
            .iter()
            .flat_map(|l| l.tiles.iter().map(|(_, blob)| blob))
    }
}

pub(crate) fn document_metadata(doc: &Document) -> usize {
    size_of::<Document>()
        + 32
        + doc.roots.capacity() * size_of::<Uuid>()
        + doc.layers.capacity() * (size_of::<(Uuid, Layer)>() + 32)
        + doc.title.as_ref().map_or(0, String::capacity)
        + doc
            .layers
            .values()
            .map(|l| l.name.capacity() + l.raster.metadata_weight())
            .sum::<usize>()
}

pub(crate) fn clone_memo(memo: &Memo, budget: &mut usize) -> Result<Memo> {
    let count = memo.iter().filter(|(k, _)| k.resident_bytes() != 0).count();
    let charge = count * (size_of::<(TileVersion, Weak<Blob>)>() + 160);
    if charge > *budget {
        return Err(limit("history version index admission exceeded"));
    }
    *budget -= charge;
    let mut copy = HashMap::new();
    copy.try_reserve(count)
        .map_err(|_| limit("cannot stage history version index"))?;
    copy.extend(
        memo.iter()
            .filter(|(k, _)| k.resident_bytes() != 0)
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    Ok(copy)
}

pub(crate) fn blob_weight(blob: &Blob) -> usize {
    size_of::<Blob>() + 32 + 128 + blob.encoded_bytes()
}

pub(crate) fn needed_sources(
    snapshot: &Snapshot,
    memo: &Memo,
    limit: usize,
    check: &impl Fn() -> Result<()>,
) -> Result<Option<usize>> {
    let mut seen = std::collections::HashSet::new();
    for layer in snapshot.document.layers.values() {
        check()?;
        for (_, lease) in layer.raster.tile_leases() {
            check()?;
            let version = lease.version();
            if memo.get(&version).and_then(Weak::upgrade).is_none() {
                seen.try_reserve(1)
                    .map_err(|_| self::limit("cannot plan history source work"))?;
                seen.insert(version);
                if seen.len() > limit {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(seen.len()))
}

pub(crate) fn corrupt() -> Error {
    payload::corrupt()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupted_recorded_patch_does_not_advance_the_history_cursor() {
        let mut doc = paintcraft_document::new_document(
            256,
            256,
            Some(Rgba([37, 19, 83, 0])),
            42,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = doc.roots[0];
        doc.layers
            .get_mut(&id)
            .unwrap()
            .raster
            .blend_pixel(
                2,
                3,
                Rgba([1, 2, 3, 255]),
                paintcraft_raster::BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let mut history = History::new(Snapshot {
            document: Arc::new(doc.clone()),
            selection: Selection::Inactive,
            history_id: None,
        });
        doc.state_id = Uuid::new_v4();
        doc.layers
            .get_mut(&id)
            .unwrap()
            .raster
            .blend_pixel(
                2,
                3,
                Rgba([4, 5, 6, 255]),
                paintcraft_raster::BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let current = doc.state_id;
        history
            .record(Snapshot {
                document: Arc::new(doc),
                selection: Selection::Inactive,
                history_id: Some(Uuid::new_v4()),
            })
            .unwrap();
        history.compact(1, 1 << 20, || Ok(())).unwrap();
        history.memo.clear();
        let Storage::Cold(packed) = &mut history.entries[0].storage else {
            panic!("expected packed state");
        };
        let packed = Arc::get_mut(packed).unwrap();
        let blob = Arc::get_mut(&mut packed.layers[0].tiles[0].1).unwrap();
        payload::corrupt_patch_for_test(blob);
        assert_eq!(history.undo().unwrap_err().code, ErrorCode::CodecFailed);
        assert_eq!(history.undo_depth(), 1);
        assert_eq!(
            history
                .entries()
                .find(|entry| entry.current)
                .unwrap()
                .state_id,
            current
        );
    }
    #[test]
    fn repeated_partial_restore_failure_reclaims_dead_aliases_without_moving_history() {
        let mut doc = paintcraft_document::new_document(
            512,
            256,
            Some(Rgba::TRANSPARENT),
            42,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = doc.roots[0];
        let raster = &mut doc.layers.get_mut(&id).unwrap().raster;
        let mut adm = Admission::new(2 * 65536);
        for x in [3, 259] {
            raster
                .blend_pixel(
                    x,
                    4,
                    Rgba([7, 8, 9, 10]),
                    paintcraft_raster::BlendMode::Replace,
                    &mut adm,
                )
                .unwrap();
        }
        let mut current = doc.clone();
        current.layers.get_mut(&id).unwrap().raster = RasterLayer::new(512, 256, Rgba::TRANSPARENT);
        current.state_id = Uuid::new_v4();
        let current_id = current.state_id;
        let mut history = History::new(Snapshot {
            document: Arc::new(doc),
            selection: Selection::Inactive,
            history_id: None,
        });
        history
            .record(Snapshot {
                document: Arc::new(current),
                selection: Selection::Inactive,
                history_id: Some(Uuid::new_v4()),
            })
            .unwrap();
        history.compact(1, 1 << 20, || Ok(())).unwrap();
        history.memo.clear();
        let Storage::Cold(packed) = &mut history.entries[0].storage else {
            panic!("expected packed state");
        };
        let packed = Arc::get_mut(packed).unwrap();
        assert!(packed.blobs().all(|b| b.source().resident_bytes() == 0));
        // The first tile decodes and registers an alias; the second then fails.
        let blob = Arc::get_mut(&mut packed.layers[0].tiles[1].1).unwrap();
        payload::corrupt_patch_for_test(blob);
        for _ in 0..128 {
            assert_eq!(history.undo().unwrap_err().code, ErrorCode::CodecFailed);
            assert!(
                history.memo.is_empty(),
                "failed private decodes must not accumulate aliases"
            );
            assert_eq!(history.undo_depth(), 1);
            assert_eq!(history.redo_depth(), 0);
            assert_eq!(
                history.entries().find(|e| e.current).unwrap().state_id,
                current_id
            );
            assert_eq!(history.memory_usage().decoded_bytes, 0);
        }
    }
}
