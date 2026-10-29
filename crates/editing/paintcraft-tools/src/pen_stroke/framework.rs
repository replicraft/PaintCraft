//! Framework contacts have positions and optional force, not native pen clocks,
//! tool identities, tilt or completion authority. They never become Samples.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameworkPoint {
    pub at: (i64, i64),
    pub force: f32,
}
impl FrameworkPoint {
    fn validate(self) -> Result<()> {
        let bound = paintcraft_core::types::policy::COORD_ABS_MAX as u64 * 256;
        if !self.force.is_finite()
            || !(0.0..=1.0).contains(&self.force)
            || self.at.0.unsigned_abs() > bound
            || self.at.1.unsigned_abs() > bound
        {
            return Err(invalid("invalid framework force point"));
        }
        Ok(())
    }
    pub(super) fn mapped(self, settings: paintcraft_core::types::pen::Settings) -> PressurePoint {
        PressurePoint {
            at: self.at,
            pressure: settings.width(Some((f64::from(self.force) * 65535.0).round() as u16)),
        }
    }
}
#[derive(Debug)]
pub(super) struct FrameworkSamples {
    pub values: Vec<FrameworkPoint>,
    pub account: BudgetAccount,
}
fn framework_bytes(capacity: usize) -> Result<u64> {
    capacity
        .checked_mul(std::mem::size_of::<FrameworkPoint>())
        .and_then(|n| {
            n.checked_add(
                std::mem::size_of::<FrameworkSamples>() + 2 * std::mem::size_of::<usize>(),
            )
        })
        .map(|n| n as u64)
        .ok_or_else(|| limit("framework force storage overflow"))
}
impl PenStroke {
    /// Captured framework-force ink, independent of the classified pen API.
    /// Only the existing plain round brush contract is admitted here.
    pub fn begin_framework(
        style: Style,
        first: FrameworkPoint,
        budget: &SharedBudget,
    ) -> Result<Self> {
        if !(1..=1024).contains(&style.diameter)
            || style.media != paintcraft_core::types::brush::BrushMedia::Plain
            || style.intent != MaskIntent::Brush
            || style.mode != MaskMode::SourceOver
        {
            return Err(invalid("unsupported framework force brush"));
        }
        first.validate()?;
        let mut native_account = budget.account(AllocationClass::PaintMetadata);
        native_account.resize(sample_bytes(0)?)?;
        let mut framework_account = budget.account(AllocationClass::PaintMetadata);
        framework_account.resize(framework_bytes(0)?)?;
        let mut stroke = Self {
            style,
            samples: Arc::new(Samples {
                values: Vec::new(),
                account: native_account,
            }),
            framework: Some(Arc::new(FrameworkSamples {
                values: Vec::new(),
                account: framework_account,
            })),
        };
        stroke.append_framework(first)?;
        Ok(stroke)
    }
    pub fn gesture_button(&self) -> Option<paintcraft_input::PointerButton> {
        if self.is_framework() {
            return Some(paintcraft_input::PointerButton::Primary);
        }
        self.samples().first().map(|s| {
            if s.identity.kind == paintcraft_input::tablet::DeviceKind::Eraser {
                paintcraft_input::PointerButton::Primary
            } else {
                s.buttons.ink_button()
            }
        })
    }
    pub fn is_framework(&self) -> bool {
        self.framework.is_some()
    }
    pub fn append_framework(&mut self, point: FrameworkPoint) -> Result<()> {
        point.validate()?;
        let storage = self
            .framework
            .as_mut()
            .ok_or_else(|| invalid("not a framework force capture"))?;
        if storage.values.len() == SAMPLES_MAX {
            return Err(limit("framework force point limit exceeded"));
        }
        let count = storage.values.len();
        if Arc::strong_count(storage) != 1 || count == storage.values.capacity() {
            let capacity = if count == storage.values.capacity() {
                storage
                    .values
                    .capacity()
                    .max(16)
                    .saturating_mul(2)
                    .min(SAMPLES_MAX)
            } else {
                storage.values.capacity()
            };
            let mut account = storage.account.sibling();
            account.resize(framework_bytes(capacity)?)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(capacity)
                .map_err(|_| limit("cannot retain framework force points"))?;
            account.resize(framework_bytes(values.capacity())?)?;
            values.extend_from_slice(&storage.values);
            *storage = Arc::new(FrameworkSamples { values, account });
        }
        Arc::get_mut(storage)
            .expect("unique admitted force points")
            .values
            .push(point);
        Ok(())
    }
    pub(super) fn point_count(&self) -> usize {
        self.framework
            .as_ref()
            .map_or(self.samples.values.len(), |f| f.values.len())
    }
    pub(super) fn point(&self, index: usize) -> Result<PressurePoint> {
        if let Some(framework) = &self.framework {
            Ok(framework.values[index].mapped(self.style.settings))
        } else {
            mapped_point(&self.samples.values[index], self.style.settings)
        }
    }
    pub(super) fn visit_framework_opacity(
        &self,
        clip: Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
        emit: &mut dyn FnMut(MaskSpan) -> Result<()>,
    ) -> Result<()> {
        use paintcraft_raster::media::dynamics::{self, Attributes, Point};
        let source = self
            .framework
            .as_ref()
            .ok_or_else(|| invalid("not framework force"))?;
        let mut account = budget.account(AllocationClass::PaintMetadata);
        let bytes = |capacity: usize| {
            capacity
                .checked_mul(std::mem::size_of::<Point>())
                .map(|n| n as u64)
                .ok_or_else(|| limit("framework force mapping overflow"))
        };
        account.resize(bytes(source.values.len())?)?;
        let mut points = Vec::new();
        points
            .try_reserve_exact(source.values.len())
            .map_err(|_| limit("cannot map force opacity"))?;
        account.resize(bytes(points.capacity())?)?;
        for (index, point) in source.values.iter().enumerate() {
            if index.is_multiple_of(256) {
                check()?;
            }
            let mapped = point.mapped(self.style.settings);
            points.push(Point {
                at: mapped.at,
                attributes: Attributes {
                    width: mapped.pressure.unwrap_or(65535),
                    opacity: self
                        .style
                        .settings
                        .curve
                        .map((f64::from(point.force) * 65535.0).round() as u16),
                    rotation_q8: 0,
                },
            });
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
