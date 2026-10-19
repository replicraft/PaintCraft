//! Read-only row segments borrow resident tiles or describe implicit pixels.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowSpan<'a> {
    /// Tightly packed straight RGBA8, borrowed without copying the tile row.
    Pixels(&'a [u8]),
    /// Missing tile: no temporary materialization or loss of hidden RGB.
    Uniform { color: Rgba, pixels: usize },
}

impl RowSpan<'_> {
    pub fn pixel_count(self) -> usize {
        match self {
            Self::Pixels(bytes) => bytes.len() / 4,
            Self::Uniform { pixels, .. } => pixels,
        }
    }
}

impl RasterLayer {
    /// Read a checked, layer-local row range without allocations. The borrow
    /// cannot outlive or mutably alias this layer; COW snapshots remain immutable.
    /// Empty ranges at the right edge are allowed, but `y` must be in bounds.
    pub fn row_spans(
        &self,
        y: u32,
        x: u32,
        pixels: u32,
    ) -> Result<impl Iterator<Item = RowSpan<'_>>> {
        if y >= self.height || x > self.width || pixels > self.width - x {
            return Err(Error::new(ErrorCode::InputSchema, "row span outside layer"));
        }
        let mut x = x;
        let end = x + pixels;
        Ok(std::iter::from_fn(move || {
            if x == end {
                return None;
            }
            let tx = x / TILE_EDGE;
            let lx = x % TILE_EDGE;
            let n = (TILE_EDGE - lx).min(end - x);
            let coord = TileCoord {
                tx: i64::from(tx),
                ty: i64::from(y / TILE_EDGE),
            };
            let span = match self.tiles.get(&coord) {
                Some(tile) => {
                    let start = ((y % TILE_EDGE) * TILE_EDGE + lx) as usize * 4;
                    RowSpan::Pixels(&tile.0[start..start + n as usize * 4])
                }
                None => RowSpan::Uniform {
                    color: self.default,
                    pixels: n as usize,
                },
            };
            x += n;
            Some(span)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_resident_and_implicit_spans_match_independent_samples_at_tile_edges() {
        let ground = Rgba([7, 8, 9, 0]);
        let ink = Rgba([40, 50, 60, 255]);
        let mut raster = RasterLayer::new(519, 259, ground);
        raster
            .blend_pixel(
                255,
                258,
                ink,
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let spans: Vec<_> = raster.row_spans(258, 254, 265).unwrap().collect();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0], RowSpan::Pixels(&[7, 8, 9, 0, 40, 50, 60, 255]));
        assert_eq!(
            spans[1],
            RowSpan::Uniform {
                color: ground,
                pixels: 256
            }
        );
        assert_eq!(
            spans[2],
            RowSpan::Uniform {
                color: ground,
                pixels: 7
            }
        );
        let clone = raster.clone();
        let mut edited = clone;
        edited
            .blend_pixel(
                255,
                258,
                ground,
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        assert_eq!(spans[0], RowSpan::Pixels(&[7, 8, 9, 0, 40, 50, 60, 255]));
        assert_eq!(edited.get(255, 258), ground);
        assert_eq!(raster.resident_tiles(), 1);
    }

    #[test]
    fn invalid_and_overflowing_ranges_are_rejected_without_iteration() {
        let raster = RasterLayer::new(3, 2, Rgba::TRANSPARENT);
        for (y, x, n) in [
            (2, 0, 1),
            (0, 4, 0),
            (0, 2, 2),
            (0, u32::MAX, 2),
            (0, 1, u32::MAX),
        ] {
            assert!(raster.row_spans(y, x, n).is_err());
        }
        assert_eq!(raster.row_spans(1, 3, 0).unwrap().count(), 0);
        assert_eq!(
            raster
                .row_spans(0, 0, 3)
                .unwrap()
                .map(RowSpan::pixel_count)
                .sum::<usize>(),
            3
        );
    }
}
