//! Weak version identities pin only Arc control blocks, not decoded pixel buffers.
use super::*;
use std::{
    hash::{Hash, Hasher},
    sync::Weak,
};

#[derive(Clone, Debug)]
pub struct TileVersion(Weak<Tile>);

impl PartialEq for TileVersion {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for TileVersion {}
impl Hash for TileVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

impl RasterLayer {
    /// Resident version identities intersecting a checked layer-local rectangle,
    /// in row-major tile order. Missing tiles are covered by the layer default.
    /// Keys cannot be reused for different allocations while a weak token exists.
    /// Arc::make_mut also detaches weak references before in-place mutation.
    pub fn tile_versions(
        &self,
        area: Rect,
    ) -> Result<impl Iterator<Item = (TileCoord, TileVersion)> + '_> {
        if area.x0 < 0
            || area.y0 < 0
            || area.x1 <= area.x0
            || area.y1 <= area.y0
            || area.x1 > i64::from(self.width)
            || area.y1 > i64::from(self.height)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "tile version region outside layer",
            ));
        }
        let edge = i64::from(TILE_EDGE);
        Ok((area.y0 / edge..=(area.y1 - 1) / edge).flat_map(move |ty| {
            (area.x0 / edge..=(area.x1 - 1) / edge).filter_map(move |tx| {
                let coord = TileCoord { tx, ty };
                self.tiles
                    .get(&coord)
                    .map(|tile| (coord, TileVersion(Arc::downgrade(tile))))
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_keys_detect_unshared_mutation_without_retaining_decoded_tiles() {
        let mut raster = RasterLayer::new(300, 300, Rgba::TRANSPARENT);
        raster
            .blend_pixel(
                3,
                4,
                Rgba([1, 2, 3, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let first = raster
            .tile_versions(raster.bounds())
            .unwrap()
            .next()
            .unwrap()
            .1;
        // One strong owner plus weak keys: mutation detaches the weak reference,
        // reusing the Vec allocation without copying/charging another pixel tile.
        raster
            .blend_pixel(
                3,
                4,
                Rgba([4, 5, 6, 255]),
                BlendMode::Replace,
                &mut Admission::new(0),
            )
            .unwrap();
        let second = raster
            .tile_versions(raster.bounds())
            .unwrap()
            .next()
            .unwrap()
            .1;
        assert_ne!(first, second);
        assert_eq!(first.0.strong_count(), 0);
        assert_eq!(second.0.strong_count(), 1);
        drop(raster);
        assert_eq!(second.0.strong_count(), 0);
    }

    #[test]
    fn identities_are_shared_by_cow_snapshots_and_only_include_intersecting_tiles() {
        let mut raster = RasterLayer::new(519, 259, Rgba::TRANSPARENT);
        let mut adm = Admission::new(3 * 65536);
        for (x, y) in [(0, 0), (256, 256), (518, 258)] {
            raster
                .blend_pixel(x, y, Rgba([1; 4]), BlendMode::Replace, &mut adm)
                .unwrap();
        }
        let before: Vec<_> = raster.tile_versions(raster.bounds()).unwrap().collect();
        let clone = raster.clone();
        assert_eq!(
            before,
            clone
                .tile_versions(clone.bounds())
                .unwrap()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            raster
                .tile_versions(Rect::new(256, 256, 1, 1).unwrap())
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            raster
                .tile_versions(Rect::new(300, 0, 3, 3).unwrap())
                .unwrap()
                .count(),
            0
        );
        assert!(
            raster
                .tile_versions(Rect::new(-1, 0, 1, 1).unwrap())
                .is_err()
        );
    }
}
