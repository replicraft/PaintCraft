//! Bounded previews sample the actual raster, never a second SVG rendering.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Raster {
    /// Nearest-neighbor inspection thumbnail, at most 256 pixels per side.
    /// Keeps the selected output's fitting and rasterization unchanged.
    pub fn preview(&self, check: &dyn Fn() -> Result<()>) -> Result<Preview> {
        check()?;
        self.options.admit(PIXELS_MAX)?;
        if self.width != self.options.width
            || self.height != self.options.height
            || self.rgba.len() as u64 != u64::from(self.width) * u64::from(self.height) * 4
        {
            return Err(malformed("SVG raster geometry changed before preview"));
        }
        let side = self.width.max(self.height);
        let width = ((u64::from(self.width) * 256) / u64::from(side)).clamp(1, 256) as u32;
        let height = ((u64::from(self.height) * 256) / u64::from(side)).clamp(1, 256) as u32;
        let (width, height) = if side <= 256 {
            (self.width, self.height)
        } else {
            (width, height)
        };
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(width as usize * height as usize * 4)
            .map_err(|_| resource("cannot allocate SVG preview"))?;
        for y in 0..height {
            check()?;
            let sy = ((u64::from(y) * 2 + 1) * u64::from(self.height) / (u64::from(height) * 2))
                as usize;
            for x in 0..width {
                let sx = ((u64::from(x) * 2 + 1) * u64::from(self.width) / (u64::from(width) * 2))
                    as usize;
                let offset = (sy * self.width as usize + sx) * 4;
                rgba.extend_from_slice(&self.rgba[offset..offset + 4]);
            }
        }
        Ok(Preview {
            width,
            height,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raster(width: u32, height: u32) -> Raster {
        Raster {
            width,
            height,
            rgba: (0..width * height)
                .flat_map(|i| [i as u8, 20, 40, 128])
                .collect(),
            losses: vec![],
            options: Options {
                width,
                height,
                dpi: 96.0,
                fit: Fit::Contain,
            },
        }
    }
    #[test]
    fn preview_samples_selected_raster_and_preserves_small_images() {
        let small = raster(32, 16);
        assert_eq!(small.preview(&|| Ok(())).unwrap().rgba, small.rgba);
        let large = raster(512, 1024);
        let preview = large.preview(&|| Ok(())).unwrap();
        assert_eq!((preview.width, preview.height), (128, 256));
        for y in 0..256 {
            for x in 0..128 {
                let original = ((y * 4 + 2) * 512 + x * 4 + 2) * 4;
                let target = (y * 128 + x) * 4;
                assert_eq!(
                    &preview.rgba[target..target + 4],
                    &large.rgba[original..original + 4]
                );
            }
        }
    }
    #[test]
    fn preview_refuses_mutated_geometry_and_honors_cancellation() {
        let mut source = raster(1, 1);
        source.width = 0;
        assert!(source.preview(&|| Ok(())).is_err());
        source.width = 1;
        source.rgba.clear();
        assert!(source.preview(&|| Ok(())).is_err());
        let error = raster(1, 1)
            .preview(&|| Err(resource("cancelled")))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
    }
}
