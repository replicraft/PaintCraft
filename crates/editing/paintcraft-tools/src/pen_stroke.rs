//! Immutable actual pen samples and a single resolved-mask publication path.
//! Named PaintCraft diameter policy; native device/driver parity is not implied.
//! Normalized pressure is rounded to nearest Q16 (ties upward); missing pressure
//! uses full diameter. Resolved 4×4 coverage becomes rounded 8-bit mask coverage.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result, Rgba,
    json::Value,
    policy::{MaskIntent, MaskMode},
    resources::{AllocationClass, BudgetAccount, SharedBudget},
    text::MaskSpan,
};
use paintcraft_input::tablet::{Phase, Provenance, Sample};
use paintcraft_raster::smooth::{self, PressurePoint};
use std::sync::Arc;
use uuid::Uuid;

const SAMPLES_MAX: usize = 65_536;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Captured plain-round style. Hosts must not convert other brush/media profiles
/// to this policy implicitly; their existing semantics remain independent.
pub struct Style {
    pub layer: Uuid,
    pub diameter: u32,
    pub color: Rgba,
    pub mode: MaskMode,
    pub intent: MaskIntent,
}
#[derive(Debug)]
struct Samples {
    values: Vec<Sample>,
    account: BudgetAccount,
}
/// Clones retain the captured style, complete source attributes and byte account.
/// Appending uses fallible copy-on-write; preview snapshots never change in place.
#[derive(Clone, Debug)]
pub struct PenStroke {
    style: Style,
    samples: Arc<Samples>,
}
#[derive(Debug)]
struct Spans {
    values: Vec<MaskSpan>,
    _account: BudgetAccount,
}
#[derive(Clone, Debug)]
pub struct ResolvedPenStroke {
    style: Style,
    spans: Arc<Spans>,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
impl PenStroke {
    pub fn begin(style: Style, first: Sample, budget: &SharedBudget) -> Result<Self> {
        if !(1..=1024).contains(&style.diameter)
            || first.phase != Phase::Down
            || !first.identity.kind.is_pen()
        {
            return Err(invalid("invalid pen stroke start or diameter"));
        }
        let mut stroke = Self {
            style,
            samples: Arc::new(Samples {
                values: Vec::new(),
                account: budget.account(AllocationClass::PaintMetadata),
            }),
        };
        stroke.append(first)?;
        Ok(stroke)
    }
    pub fn style(&self) -> Style {
        self.style
    }
    pub fn samples(&self) -> &[Sample] {
        &self.samples.values
    }
    pub fn declared_bytes(&self) -> u64 {
        self.samples.account.bytes()
    }
    pub fn append(&mut self, sample: Sample) -> Result<()> {
        sample
            .validate()
            .map_err(|_| invalid("invalid pen source sample"))?;
        mapped_point(&sample)?;
        if matches!(
            sample.provenance,
            Provenance::Predicted | Provenance::Boundary
        ) || !matches!(sample.phase, Phase::Down | Phase::Move | Phase::Up)
        {
            return Err(invalid("only actual/coalesced ink samples enter a stroke"));
        }
        if let Some(last) = self.samples.values.last() {
            if sample == *last {
                return Ok(());
            }
            if sample.identity != last.identity
                || sample.transform.owner != last.transform.owner
                || sample.time.clock != last.time.clock
                || sample.time.epoch != last.time.epoch
                || sample.sequence <= last.sequence
                || sample.time.micros < last.time.micros
                || last.phase == Phase::Up
                || sample.phase == Phase::Down
            {
                return Err(invalid("pen capture, clock or phase changed during stroke"));
            }
        }
        let count = self.samples.values.len();
        if count == SAMPLES_MAX {
            return Err(limit("pen stroke sample limit exceeded"));
        }
        let shared = Arc::strong_count(&self.samples) != 1;
        if shared || count == self.samples.values.capacity() {
            let previous = self.samples.values.capacity();
            let capacity = if count == previous {
                previous.max(16).saturating_mul(2).min(SAMPLES_MAX)
            } else {
                previous
            };
            let mut account = self.samples.account.sibling();
            account.resize(sample_bytes(capacity)?)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(capacity)
                .map_err(|_| limit("cannot retain pen samples"))?;
            account.resize(sample_bytes(values.capacity())?)?;
            values.extend_from_slice(&self.samples.values);
            self.samples = Arc::new(Samples { values, account });
        }
        Arc::get_mut(&mut self.samples)
            .expect("unique admitted samples")
            .values
            .push(sample);
        Ok(())
    }
    /// One union mask is shared by preview and commit. Hosts retain this result
    /// through their existing staged publication barrier and history transaction.
    pub fn resolve(
        &self,
        clip: Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ResolvedPenStroke> {
        check()?;
        let count = self.samples.values.len();
        // Admit scanner scratch before mapping or geometry. Row ranges remain
        // live across the path, but the event tree exists for one pixel row only.
        // Output mask capacity is admitted incrementally by the same resolver.
        // These are logical declarations, not allocator/RSS interception.
        let quote = self.scan_quote(clip, check)?;
        let mut scratch = budget.account(AllocationClass::PaintMetadata);
        scratch.resize(quote as u64)?;
        let mut points = Vec::new();
        points
            .try_reserve_exact(count)
            .map_err(|_| limit("cannot map pen samples"))?;
        for chunk in self.samples.values.chunks(256) {
            check()?;
            for sample in chunk {
                let point = mapped_point(sample)?;
                if points
                    .last()
                    .is_none_or(|last| !same_geometry(*last, point))
                {
                    points.push(point);
                }
            }
        }
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(span_bytes(0)?)?;
        let mut values = Vec::new();
        let mut pixels = 0u64;
        smooth::pressure_path_visit_checked(
            &points,
            self.style.diameter,
            &clip,
            check,
            &mut |span| {
                check()?;
                pixels = pixels
                    .checked_add((span.x1 - span.x0 + 1) as u64)
                    .ok_or_else(|| limit("pen mask pixel quote overflow"))?;
                if values.len() == 262_144 || pixels > 16_777_216 {
                    return Err(limit("resolved pen mask allowance exceeded"));
                }
                if values.len() == values.capacity() {
                    let capacity = values.capacity().max(16).saturating_mul(2).min(262_144);
                    account.resize(span_bytes(capacity)?)?;
                    values
                        .try_reserve_exact(capacity - values.len())
                        .map_err(|_| limit("cannot retain pen mask"))?;
                    account.resize(span_bytes(values.capacity())?)?;
                }
                values.push(MaskSpan {
                    x0: span.x0,
                    y: span.y,
                    x1: span.x1,
                    coverage: ((u16::from(span.samples) * 255 + 8) / 16) as u8,
                });
                Ok(())
            },
        )?;
        check()?;
        Ok(ResolvedPenStroke {
            style: self.style,
            spans: Arc::new(Spans {
                values,
                _account: account,
            }),
        })
    }
    fn scan_quote(&self, clip: Rect, check: &dyn Fn() -> Result<()>) -> Result<usize> {
        check()?;
        let bound = paintcraft_core::types::policy::COORD_ABS_MAX as u64;
        if [clip.x0, clip.y0, clip.x1, clip.y1]
            .iter()
            .any(|v| v.unsigned_abs() > bound)
        {
            return Err(invalid("invalid pen clip coordinates"));
        }
        let mapped_bytes = self
            .samples
            .values
            .len()
            .checked_mul(std::mem::size_of::<PressurePoint>())
            .ok_or_else(|| limit("pen map quote overflow"))?;
        if clip.width() <= 0 || clip.height() <= 0 {
            return Ok(mapped_bytes);
        }
        let bounds = |p: PressurePoint| {
            let r = f64::from(self.style.diameter) * f64::from(p.pressure.unwrap_or(65535))
                / 65535.0
                / 2.0;
            let y = (p.at.1 - clip.y0 * 256) as f64 / 256.0;
            (
                ((y - r) * 4.0).floor().max(0.0),
                ((y + r) * 4.0).ceil().min(clip.height() as f64 * 4.0),
            )
        };
        let (mut previous, mut first) = (None, None);
        let (mut rows, mut geometry) = (0usize, 0usize);
        let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
        for (index, sample) in self.samples.values.iter().enumerate() {
            if index.is_multiple_of(256) {
                check()?;
            }
            let point = mapped_point(sample)?;
            if previous.is_some_and(|p| same_geometry(p, point)) {
                continue;
            }
            geometry += 1;
            let extent = bounds(point);
            top = top.min(extent.0);
            bottom = bottom.max(extent.1);
            if let Some(old) = previous {
                let old = bounds(old);
                rows = rows
                    .checked_add((old.1.max(extent.1) - old.0.min(extent.0)).max(0.0) as usize)
                    .ok_or_else(|| limit("pen row quote overflow"))?;
            } else {
                first = Some(extent);
            }
            if rows > smooth::SCAN_ROWS_MAX {
                return Err(limit("pen scan row budget exceeded"));
            }
            previous = Some(point);
        }
        if geometry == 1 {
            let (a, b) = first.expect("captured sample");
            rows = (b - a).max(0.0) as usize;
        }
        let distinct_rows = rows.min((bottom - top).max(0.0) as usize);
        // Range Vec capacity <=2R+4*distinct_rows (including its small initial
        // allocation). Tree entries use the existing PathCache 128-byte logical
        // allowance. A pixel row has <=4*geometry intervals and <=16*geometry
        // event keys, independent of the number of earlier rows already resolved.
        rows.checked_mul(32)
            .and_then(|n| {
                distinct_rows
                    .checked_mul(192)
                    .and_then(|d| n.checked_add(d))
            })
            .and_then(|n| geometry.checked_mul(2048).and_then(|e| n.checked_add(e)))
            .and_then(|n| n.checked_add(mapped_bytes))
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| limit("pen geometry quote overflow"))
    }
}
fn same_geometry(a: PressurePoint, b: PressurePoint) -> bool {
    a.at == b.at && a.pressure.unwrap_or(65535) == b.pressure.unwrap_or(65535)
}
fn mapped_point(sample: &Sample) -> Result<PressurePoint> {
    let at = sample
        .q8()
        .map_err(|_| invalid("pen coordinates exceed fixed-point range"))?;
    let bound = paintcraft_core::types::policy::COORD_ABS_MAX as u64 * 256;
    if at.0.unsigned_abs() > bound || at.1.unsigned_abs() > bound {
        return Err(invalid("pen coordinates exceed raster policy"));
    }
    Ok(PressurePoint {
        at,
        pressure: sample
            .pressure
            .map(|p| (f64::from(p) * 65535.0).round() as u16),
    })
}
fn sample_bytes(capacity: usize) -> Result<u64> {
    capacity
        .checked_mul(std::mem::size_of::<Sample>())
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<Samples>() + 2 * std::mem::size_of::<usize>())
        })
        .map(|n| n as u64)
        .ok_or_else(|| limit("pen sample quote overflow"))
}
fn span_bytes(capacity: usize) -> Result<u64> {
    capacity
        .checked_mul(std::mem::size_of::<MaskSpan>())
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<Spans>() + 2 * std::mem::size_of::<usize>())
        })
        .map(|n| n as u64)
        .ok_or_else(|| limit("pen mask quote overflow"))
}
impl ResolvedPenStroke {
    pub fn style(&self) -> Style {
        self.style
    }
    pub fn spans(&self) -> &[MaskSpan] {
        &self.spans.values
    }
    pub fn operations(&self, check: &impl Fn() -> Result<()>) -> Result<Vec<Value>> {
        crate::commands::mask_operations_with_mode(
            self.style.layer,
            self.style.color,
            &self.spans.values,
            self.style.mode,
            check,
        )
    }
}

impl PartialEq for PenStroke {
    fn eq(&self, other: &Self) -> bool {
        self.style == other.style && self.samples.values == other.samples.values
    }
}
impl PenStroke {
    /// Use the captured source's shared metadata realm for geometry and masks.
    pub fn resolve_captured(
        &self,
        clip: Rect,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ResolvedPenStroke> {
        self.resolve(clip, &self.samples.account.budget(), check)
    }
}
