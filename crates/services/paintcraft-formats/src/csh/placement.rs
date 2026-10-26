//! Explicit bounded CSH-to-mask conversion using the existing tiny-skia filler.
//! Source coordinates stay exact; the rasterizer receives f32 coordinates.
use super::*;
use paintcraft_core::types::{
    Rgba,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
    text::MaskSpan,
};
use resvg::tiny_skia;
use std::sync::Arc;
pub const SIDE_MAX: u32 = 4096;
pub const PIXELS_MAX: u64 = 4_194_304;
pub const KNOTS_MAX: usize = 256;
pub const SPANS_MAX: usize = 262_144;
const UNIT: i64 = 1 << 24;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    CubicFillV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    MissingPath,
    MultiplePaths,
    OpenPath,
    BooleanOperation,
    InitialFill,
    Clipboard,
    KnotCount,
    CoordinateRange,
    MalformedTopology,
}
#[derive(Clone, Debug)]
pub struct Assessment {
    unsupported: Vec<Unsupported>,
}
impl Assessment {
    pub fn qualified(&self) -> bool {
        self.unsupported.is_empty()
    }
    pub fn unsupported(&self) -> &[Unsupported] {
        &self.unsupported
    }
    fn refuse(&mut self, reason: Unsupported) {
        if !self.unsupported.contains(&reason) {
            self.unsupported.push(reason);
        }
    }
}
/// Assessment does not change the library, create a document or evaluate pixels.
pub fn assess(shape: &Shape, check: &dyn Fn() -> Result<()>) -> Result<Assessment> {
    check()?;
    let mut assessment = Assessment {
        unsupported: Vec::new(),
    };
    let mut paths = 0usize;
    let mut declared = 0usize;
    let mut knots = 0usize;
    for record in &shape.records {
        check()?;
        match record {
            Record::Subpath {
                open,
                knots: count,
                operation,
                ..
            } => {
                paths += 1;
                declared += usize::from(*count);
                if *open {
                    assessment.refuse(Unsupported::OpenPath);
                }
                if !matches!(operation, None | Some(Operation::Combine)) {
                    assessment.refuse(Unsupported::BooleanOperation);
                }
            }
            Record::Knot { open, points, .. } => {
                knots += 1;
                if *open {
                    assessment.refuse(Unsupported::OpenPath);
                }
                if points
                    .iter()
                    .any(|value| !(-UNIT..=2 * UNIT).contains(&i64::from(*value)))
                {
                    assessment.refuse(Unsupported::CoordinateRange);
                }
                if paths == 0 {
                    assessment.refuse(Unsupported::MalformedTopology);
                }
            }
            Record::InitialFill(true) => assessment.refuse(Unsupported::InitialFill),
            Record::Clipboard(_) => assessment.refuse(Unsupported::Clipboard),
            Record::FillRule(rule) if *rule != 0 => {
                assessment.refuse(Unsupported::MalformedTopology)
            }
            _ => {}
        }
    }
    if paths == 0 {
        assessment.refuse(Unsupported::MissingPath);
    } else if paths != 1 {
        assessment.refuse(Unsupported::MultiplePaths);
    }
    if !(2..=KNOTS_MAX).contains(&knots) {
        assessment.refuse(Unsupported::KnotCount);
    }
    if knots != declared {
        assessment.refuse(Unsupported::MalformedTopology);
    }
    check()?;
    Ok(assessment)
}
#[derive(Clone, Debug)]
pub struct Rasterized {
    source: Arc<Library>,
    shape: usize,
    dimensions: (u32, u32),
    policy: Policy,
    coverage: Coverage,
}
impl Rasterized {
    pub fn source(&self) -> &Arc<Library> {
        &self.source
    }
    pub fn shape_index(&self) -> usize {
        self.shape
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
    pub fn policy(&self) -> Policy {
        self.policy
    }
    pub fn spans(&self) -> &[MaskSpan] {
        self.coverage.spans()
    }
    pub fn translated_spans(
        &self,
        at: (i64, i64),
        clip: paintcraft_core::types::Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Coverage> {
        use paintcraft_core::types::policy::COORD_ABS_MAX;
        check()?;
        let right = at.0.checked_add(i64::from(self.dimensions.0));
        let bottom = at.1.checked_add(i64::from(self.dimensions.1));
        if [Some(at.0), Some(at.1), right, bottom]
            .iter()
            .any(|value| value.is_none_or(|v| v.unsigned_abs() > COORD_ABS_MAX as u64))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "CSH placement coordinates exceed policy",
            ));
        }
        if [clip.x0, clip.y0, clip.x1, clip.y1]
            .iter()
            .any(|value| value.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "CSH placement clip exceeds policy",
            ));
        }
        let mut storage = SpanStorage::new(budget);
        if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
            return Ok(Coverage(Arc::new(storage)));
        }
        for (index, span) in self.spans().iter().enumerate() {
            if index.is_multiple_of(256) {
                check()?;
            }
            let y = span.y + at.1;
            let x0 = (span.x0 + at.0).max(clip.x0);
            let x1 = (span.x1 + at.0).min(clip.x1 - 1);
            if y >= clip.y0 && y < clip.y1 && x0 <= x1 {
                storage.push(MaskSpan {
                    y,
                    x0,
                    x1,
                    coverage: span.coverage,
                })?;
            }
        }
        check()?;
        Ok(Coverage(Arc::new(storage)))
    }
    pub fn rgba(
        &self,
        color: Rgba,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Preview> {
        let (width, height) = self.dimensions;
        let length = width as usize * height as usize * 4;
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(length as u64)?;
        check()?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length)
            .map_err(|_| resource("cannot reserve CSH preview"))?;
        account.resize(rgba.capacity() as u64)?;
        rgba.resize(length, 0);
        for span in self.spans() {
            check()?;
            for x in span.x0..=span.x1 {
                let at = (span.y as usize * width as usize + x as usize) * 4;
                rgba[at..at + 4].copy_from_slice(&[
                    color.0[0],
                    color.0[1],
                    color.0[2],
                    ((u16::from(color.a()) * u16::from(span.coverage) + 127) / 255) as u8,
                ]);
            }
        }
        check()?;
        Ok(Preview { rgba, account })
    }
}
/// Owned span storage and admission; clones retain one allocation and charge.
#[derive(Clone, Debug)]
pub struct Coverage(Arc<SpanStorage>);
impl Coverage {
    pub fn spans(&self) -> &[MaskSpan] {
        &self.0.spans
    }
    pub fn declared_bytes(&self) -> u64 {
        self.0.account.bytes()
    }
}
#[derive(Debug)]
struct SpanStorage {
    spans: Vec<MaskSpan>,
    account: BudgetAccount,
}
impl SpanStorage {
    fn new(budget: &SharedBudget) -> Self {
        Self {
            spans: Vec::new(),
            account: budget.account(AllocationClass::PaintMetadata),
        }
    }
    fn push(&mut self, span: MaskSpan) -> Result<()> {
        if self.spans.len() == SPANS_MAX {
            return Err(resource("CSH placement spans exceed allowance"));
        }
        if self.spans.len() == self.spans.capacity() {
            let wanted = (self.spans.capacity().max(8) * 2).min(SPANS_MAX);
            self.account
                .resize((wanted * std::mem::size_of::<MaskSpan>()) as u64)?;
            self.spans
                .try_reserve_exact(wanted - self.spans.len())
                .map_err(|_| resource("cannot reserve CSH placement spans"))?;
            self.account
                .resize((self.spans.capacity() * std::mem::size_of::<MaskSpan>()) as u64)?;
        }
        self.spans.push(span);
        Ok(())
    }
}
/// Own preview bytes with their admission lease. A frontend retaining another
/// image copy must also retain `declare_copy`'s lease for that owned copy.
#[derive(Debug)]
pub struct Preview {
    rgba: Vec<u8>,
    account: BudgetAccount,
}
impl Preview {
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn declared_bytes(&self) -> u64 {
        self.account.bytes()
    }
    pub fn declare_copy(&self, budget: &SharedBudget) -> Result<BudgetAccount> {
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(self.rgba.len() as u64)?;
        Ok(account)
    }
}
/// One closed path, filled at an explicit output resolution. No path Boolean
/// evaluation or curve flattening substitutes for unsupported source behavior.
/// tiny-skia calls are bounded here but are not interruptible inside a fill.
pub fn rasterize(
    source: Arc<Library>,
    shape: usize,
    width: u32,
    height: u32,
    policy: Policy,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Rasterized> {
    check()?;
    if width == 0
        || height == 0
        || width > SIDE_MAX
        || height > SIDE_MAX
        || u64::from(width) * u64::from(height) > PIXELS_MAX
    {
        return Err(resource("CSH placement dimensions exceed allowance"));
    }
    let preset = source.shapes().get(shape).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "CSH shape index is outside its library",
        )
    })?;
    let assessment = assess(preset, check)?;
    if !assessment.qualified() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            format!("Unsupported CSH placement: {:?}", assessment.unsupported),
        ));
    }
    // Provisional declaration for admitted input/path vectors plus the pixel
    // scratch buffer. Internal tiny-skia allocations are not a hard RSS bound.
    let mut scratch = budget.account(AllocationClass::PaintMetadata);
    let knots_count = preset
        .records
        .iter()
        .filter(|record| matches!(record, Record::Knot { .. }))
        .count();
    scratch.resize(u64::from(width) * u64::from(height) * 4 + knots_count as u64 * 256)?;
    check()?;
    let mut knots = Vec::new();
    let mut fill_rule = PathFillRule::EvenOdd;
    for record in &preset.records {
        check()?;
        match record {
            Record::Subpath {
                fill_rule: rule, ..
            } => fill_rule = *rule,
            Record::Knot { points, .. } => knots.push(*points),
            _ => {}
        }
    }
    let coordinate = |pair: &[i32]| {
        (
            map_coordinate(pair[0], width),
            map_coordinate(pair[1], height),
        )
    };
    let mut path = tiny_skia::PathBuilder::new();
    let first = coordinate(&knots[0][2..4]);
    path.move_to(first.0, first.1);
    for (previous, next) in knots
        .iter()
        .zip(knots.iter().cycle().skip(1))
        .take(knots.len())
    {
        check()?;
        let a = coordinate(&previous[4..6]);
        let b = coordinate(&next[0..2]);
        let c = coordinate(&next[2..4]);
        path.cubic_to(a.0, a.1, b.0, b.1, c.0, c.1);
    }
    path.close();
    let path = path
        .finish()
        .ok_or_else(|| invalid("CSH placement path is degenerate"))?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| resource("cannot reserve CSH placement pixels"))?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;
    let fill_rule = match fill_rule {
        PathFillRule::NonZero => tiny_skia::FillRule::Winding,
        _ => tiny_skia::FillRule::EvenOdd,
    };
    check()?;
    pixmap.fill_path(
        &path,
        &paint,
        fill_rule,
        tiny_skia::Transform::identity(),
        None,
    );
    check()?;
    let mut storage = SpanStorage::new(budget);
    for (y, row) in pixmap.data().chunks_exact(width as usize * 4).enumerate() {
        check()?;
        let mut x = 0usize;
        while x < width as usize {
            let coverage = row[x * 4 + 3];
            let start = x;
            x += 1;
            while x < width as usize && row[x * 4 + 3] == coverage {
                x += 1;
            }
            if coverage != 0 {
                storage.push(MaskSpan {
                    y: y as i64,
                    x0: start as i64,
                    x1: x as i64 - 1,
                    coverage,
                })?;
            }
        }
    }
    check()?;
    Ok(Rasterized {
        source,
        shape,
        dimensions: (width, height),
        policy,
        coverage: Coverage(Arc::new(storage)),
    })
}
fn map_coordinate(value: i32, extent: u32) -> f32 {
    (f64::from(value) * f64::from(extent) / UNIT as f64) as f32
}
#[cfg(test)]
mod tests;
