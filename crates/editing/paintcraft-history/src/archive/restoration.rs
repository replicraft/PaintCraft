//! Pin reusable versions and reserve every missing unique blob before decoding.
use super::*;
use paintcraft_raster::TileLease;
#[derive(Debug)]
pub(super) struct Plan {
    tiles: HashMap<usize, Option<TileLease>>,
}
impl Plan {
    pub(super) fn capture(
        packed: &Packed,
        admission: &mut Admission,
        check: &impl Fn() -> Result<()>,
    ) -> Result<Self> {
        let count = packed.layers.iter().try_fold(0usize, |total, layer| {
            total
                .checked_add(layer.tiles.len())
                .ok_or_else(|| limit("restore plan size overflow"))
        })?;
        let mut tiles = HashMap::new();
        tiles
            .try_reserve(count)
            .map_err(|_| limit("cannot allocate restore plan"))?;
        let mut missing = 0u64;
        for layer in &packed.layers {
            for (_, blob) in &layer.tiles {
                check()?;
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    tiles.entry(Arc::as_ptr(blob) as usize)
                {
                    let resident = blob.resident();
                    if resident.is_none() {
                        missing = missing
                            .checked_add(1)
                            .ok_or_else(|| limit("restore plan tile overflow"))?;
                    }
                    entry.insert(resident);
                }
            }
        }
        check()?;
        admission.reserve_tiles(missing)?;
        Ok(Self { tiles })
    }
    pub(super) fn len(&self) -> usize {
        self.tiles.len()
    }
    pub(super) fn tile(
        &mut self,
        blob: &Arc<Blob>,
        admission: &mut Admission,
    ) -> Result<TileLease> {
        let resident = self
            .tiles
            .get_mut(&(Arc::as_ptr(blob) as usize))
            .expect("planned blob");
        if resident.is_none() {
            *resident = Some(blob.restore(admission)?);
        }
        Ok(resident.as_ref().expect("materialized blob").clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::SharedBudget;
    const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
    fn source(budget: &SharedBudget) -> TileLease {
        TileLease::materialize(
            Rgba([13, 29, 41, 0]),
            &mut Admission::with_residency(65_536, budget.clone()),
            |_| Ok(()),
        )
        .unwrap()
    }
    fn blob(source: TileLease) -> Arc<Blob> {
        Arc::new(
            Blob::pack(
                source,
                Rgba::TRANSPARENT,
                &mut (4 << 20),
                &|| Ok(()),
                &mut compression::Encoder::new(Compression::None),
            )
            .unwrap(),
        )
    }
    fn packed(blob: Arc<Blob>) -> Packed {
        let doc = paintcraft_document::new_document(
            512,
            256,
            Some(Rgba::TRANSPARENT),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = doc.roots[0];
        Packed {
            document_id: doc.document_id,
            state_id: doc.state_id,
            canvas: doc.canvas,
            roots: doc.roots,
            title: doc.title,
            source_metadata: doc.source_metadata,
            selection: Selection::Inactive,
            history_id: None,
            decoded_doc: Mutex::new(Weak::new()),
            layers: vec![SavedLayer {
                id,
                name: "shared".into(),
                origin: (0, 0),
                visible: true,
                opacity: 255,
                filters: Default::default(),
                width: 512,
                height: 256,
                default: Rgba::TRANSPARENT,
                tiles: vec![
                    (TileCoord { tx: 0, ty: 0 }, blob.clone()),
                    (TileCoord { tx: 1, ty: 0 }, blob),
                ],
            }],
        }
    }
    #[test]
    fn duplicate_blob_coordinates_share_one_planned_allocation_and_preserve_hidden_rgb() {
        let original = SharedBudget::new(BYTES);
        let packed = packed(blob(source(&original)));
        assert_eq!(original.usage().reserved, 0);
        let budget = SharedBudget::new(BYTES);
        let mut admission = Admission::with_residency(65_536, budget.clone());
        let snapshot = packed
            .restore(&mut admission, &mut Memo::new(), &|| Ok(()))
            .unwrap();
        assert_eq!(budget.usage().reserved, BYTES);
        let raster = &snapshot.document.layers[&snapshot.document.roots[0]].raster;
        assert_eq!(raster.resident_tiles(), 2);
        for x in [0, 255, 256, 511] {
            assert_eq!(raster.get(x, 8), Rgba([13, 29, 41, 0]));
        }
        let versions: Vec<_> = raster
            .tile_leases()
            .map(|(_, lease)| lease.version())
            .collect();
        assert_eq!(versions[0], versions[1]);
        drop((snapshot, admission));
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn reusable_tiles_remain_pinned_when_the_original_reader_drops_after_planning() {
        let original = SharedBudget::new(BYTES);
        let reader = source(&original);
        let blob = blob(reader.clone());
        let packed = packed(blob.clone());
        let budget = SharedBudget::new(0);
        let mut admission = Admission::with_residency(0, budget.clone());
        let mut plan = Plan::capture(&packed, &mut admission, &|| Ok(())).unwrap();
        assert_eq!(plan.len(), 1);
        drop(reader);
        assert_eq!(original.usage().reserved, BYTES);
        let restored = plan.tile(&blob, &mut admission).unwrap();
        drop(plan);
        assert_eq!(original.usage().reserved, BYTES);
        assert_eq!(budget.usage().reserved, 0);
        drop(restored);
        assert_eq!(original.usage().reserved, 0);
    }
}
