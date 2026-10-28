//! Captured sample-to-tip mapping. Raw source records are never rewritten.
use super::*;
use paintcraft_core::types::{brush::BrushMedia, pen::Orientation};
use paintcraft_input::tablet::Tilt;
use paintcraft_raster::media::dynamics::{self, Attributes, Point};

impl PenStroke {
    pub(super) fn visit_dynamics(
        &self,
        clip: Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
        emit: &mut dyn FnMut(MaskSpan) -> Result<()>,
    ) -> Result<()> {
        check()?;
        let bytes = |capacity: usize| {
            capacity
                .checked_mul(std::mem::size_of::<Point>())
                .map(|n| n as u64)
                .ok_or_else(|| limit("pen dynamics map quote overflow"))
        };
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(bytes(self.samples.values.len())?)?;
        let mut points = Vec::new();
        points
            .try_reserve_exact(self.samples.values.len())
            .map_err(|_| limit("cannot map captured pen dynamics"))?;
        account.resize(bytes(points.capacity())?)?;
        for chunk in self.samples.values.chunks(256) {
            check()?;
            for sample in chunk {
                let mapped = mapped_point(sample, self.style.settings)?;
                let pressure = sample.pressure.map(|p| {
                    self.style
                        .settings
                        .curve
                        .map((f64::from(p) * 65535.0).round() as u16)
                });
                let point = Point {
                    at: mapped.at,
                    attributes: Attributes {
                        width: mapped.pressure.unwrap_or(65535),
                        opacity: if self.style.settings.opacity_pressure {
                            pressure.unwrap_or(65535)
                        } else {
                            65535
                        },
                        rotation_q8: rotation(
                            sample,
                            self.style.media,
                            self.style.settings.orientation,
                        ),
                    },
                };
                if points.last() != Some(&point) {
                    points.push(point);
                }
            }
        }
        dynamics::visit_checked(
            &points,
            self.style.diameter,
            self.style.media,
            clip,
            budget,
            check,
            emit,
        )
    }
}

fn rotation(sample: &Sample, media: BrushMedia, orientation: Orientation) -> i32 {
    if !dynamics::supports_orientation(media) {
        return 0;
    }
    let angle = match orientation {
        Orientation::Historical => None,
        Orientation::Twist => sample.twist_degrees.map(f64::from),
        Orientation::Tilt => sample.tilt.and_then(|tilt| {
            // The normalized API convention is projected angles divided by
            // 90, not a unit azimuth vector. This agrees with Chromium's AppKit
            // conversion for its flipped view; it is not a hardware oracle.
            let (x, y) = match tilt {
                Tilt::Normalized(x, y) => (f64::from(x) * 90.0, f64::from(y) * 90.0),
                Tilt::Degrees(x, y) => (f64::from(x), f64::from(y)),
            };
            // Exact upright and non-axis horizontal projections do not establish
            // azimuth. Keep the historical nib rather than inventing one. Small
            // nonzero reported tilts remain meaningful; there is no dead zone.
            if (x == 0.0 && y == 0.0)
                || ((x.abs() == 90.0 || y.abs() == 90.0) && x != 0.0 && y != 0.0)
            {
                return None;
            }
            let (x, y) = (x.to_radians(), y.to_radians());
            // atan2(tan(y),tan(x)) without infinities at ±90 degrees.
            let (x, y) = (x.sin() * y.cos(), y.sin() * x.cos());
            Some(y.atan2(x).to_degrees())
        }),
    };
    let Some(angle) = angle else {
        return 0;
    };
    let historical = match media {
        BrushMedia::CalligraphyBrush => 45.0,
        BrushMedia::CalligraphyPen => -45.0,
        _ => 0.0,
    };
    let relative = ((angle - historical) * 256.0).round() as i32;
    (relative + 46080).rem_euclid(92160) - 46080
}
