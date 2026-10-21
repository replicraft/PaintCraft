use super::*;
impl History {
    /// Each batch bounds pixel scans and writes, while the complete candidate
    /// remains private. This synchronous continuation is not a latency bound.
    pub(super) fn relieve_pressure(&mut self, check: &impl Fn() -> Result<()>) -> Result<()> {
        let mut keep_hot = 2;
        while self.memory_usage().retained_weight() > self.budget {
            check()?;
            let mut packed = 0;
            if self.packing == Packing::Adaptive && self.budget != 0 {
                match self.compact_limited(keep_hot, 64 << 20, 64, 64, check) {
                    Ok(report) => packed = report.packed_entries,
                    Err(error)
                        if error.code == paintcraft_core::types::ErrorCode::OperationCancelled =>
                    {
                        return Err(error);
                    }
                    Err(error) => self.last_packing_error = Some(error),
                }
            }
            let spilled = if self.spill_store.is_some() {
                self.spill_unpinned(64 << 20, 64, check)?.payloads
            } else {
                self.refine_deferred(64 << 20, 64, check)?.payloads
            };
            if self.memory_usage().retained_weight() <= self.budget {
                break;
            }
            if packed == 0 && spilled == 0 {
                if keep_hot == 2 && self.packing == Packing::Adaptive {
                    keep_hot = 1;
                    continue;
                }
                return Err(self.last_packing_error.clone().unwrap_or_else(||archive::limit("cannot preserve undo history within its allowance; save and reopen, increase the allowance, or explicitly choose trimming")));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::Rgba;
    use paintcraft_document::import_image;
    use paintcraft_raster::{Admission, BlendMode, Selection};
    #[test]
    fn bounded_preparation_registers_large_snapshots_without_scanning_every_tile() {
        let width = 80 * 256;
        let mut seed = 42u32;
        let pixels: Vec<u8> = (0..width * 256)
            .flat_map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed.to_le_bytes()
            })
            .collect();
        let doc = import_image(width, 256, &pixels, "bounded-large-history").unwrap();
        let mut next = doc.clone();
        let id = next.roots[0];
        next.layers
            .get_mut(&id)
            .unwrap()
            .raster
            .blend_pixel(
                3,
                5,
                Rgba([23, 88, 156, 204]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        next.state_id = Uuid::new_v4();
        let mut history = History::new(Snapshot {
            document: Arc::new(doc),
            selection: Selection::Inactive,
            history_id: None,
        });
        history
            .record(Snapshot {
                document: Arc::new(next),
                selection: Selection::Inactive,
                history_id: None,
            })
            .unwrap();
        let report = history
            .compact_limited(1, 64 << 20, 64, 64, || Ok(()))
            .unwrap();
        assert_eq!(report.packed_entries, 1);
        assert_eq!(report.scanned_sources, 64);
        assert_eq!(report.deferred_sources, 16);
        history.set_restore_budget(0);
        let restored = history.undo().unwrap().unwrap();
        assert_eq!(restored.document.layers[&id].raster.resident_tiles(), 80);
        assert_eq!(
            restored.document.layers[&id].raster.get(3, 5).0,
            pixels[(5 * width as usize + 3) * 4..(5 * width as usize + 4) * 4]
        );
    }
}
