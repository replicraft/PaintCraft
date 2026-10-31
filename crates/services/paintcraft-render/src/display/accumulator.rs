//! Resumable display reduction of already composed native samples.
use super::*;

/// The same encoded-RGB, alpha-weighted reduction as ordinary CPU/GPU display.
/// This small state can survive between Worker steps; no image is allocated.
#[derive(Clone, Copy, Debug, Default)]
pub struct DisplayAccumulator {
    sum: Sum,
    samples: u64,
}
impl DisplayAccumulator {
    /// Add one complete native composite pixel, including transparent samples.
    /// The checked renderer bound keeps all u64 channel sums representable.
    pub fn add(&mut self, pixel: Rgba) -> Result<()> {
        let samples = self
            .samples
            .checked_add(1)
            .filter(|count| *count <= RENDER_PIXELS_MAX)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Display sample accumulation exceeds its bound",
                )
            })?;
        self.sum.add(pixel, 1);
        self.samples = samples;
        Ok(())
    }
    pub fn samples(&self) -> u64 {
        self.samples
    }
    /// Empty cells are invalid. Each partial state retains exact unrounded sums.
    pub fn resolve(&self) -> Result<Rgba> {
        if self.samples == 0 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Display cell has no native samples",
            ));
        }
        Ok(Rgba(self.sum.pixel(self.samples)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_native_steps_match_one_alpha_weighted_reduction() {
        let mut value = DisplayAccumulator::default();
        assert_eq!(value.resolve().unwrap_err().code, ErrorCode::InputSchema);
        value.add(Rgba([255, 0, 0, 255])).unwrap();
        let mut resumed = value;
        resumed.add(Rgba([0, 255, 255, 0])).unwrap();
        assert_eq!(resumed.samples(), 2);
        assert_eq!(resumed.resolve().unwrap(), Rgba([255, 0, 0, 128]));
        value.add(Rgba([0, 200, 0, 255])).unwrap();
        assert_eq!(value.resolve().unwrap(), Rgba([128, 100, 0, 255]));
    }
    #[test]
    fn rejected_growth_does_not_change_a_completed_cell() {
        let mut value = DisplayAccumulator {
            sum: Sum([0; 4]),
            samples: RENDER_PIXELS_MAX,
        };
        let before = value;
        assert_eq!(
            value.add(Rgba([255; 4])).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(value.samples, before.samples);
        assert_eq!(value.sum.0, before.sum.0);
    }
}
