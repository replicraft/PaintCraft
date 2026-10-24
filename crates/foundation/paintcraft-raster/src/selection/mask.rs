//! Immutable finite binary R8 fields. A mask is always active, including all-zero fields.
use crate::{TileCoord, tile_split};
use paintcraft_core::types::resources::{BudgetAccount, SharedBudget};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result, policy::COORD_ABS_MAX, text::MaskSpan,
};
use std::{collections::HashMap, sync::Arc};

mod bounds;
mod builder;
pub use bounds::{BOUNDS_CACHE_WORK, BoundsCache};
mod outline;
pub use outline::{BoundarySegment, MaskOutline, OutlineLimits};
mod polygon;
mod rows;
mod transform;
mod views;

/// Explicit per-construction bounds, separate from the caller's shared byte capacity.
#[derive(Clone, Copy, Debug)]
pub struct MaskLimits {
    pub work: u64,
    pub tiles: usize,
    pub spans: usize,
}
impl Default for MaskLimits {
    fn default() -> Self {
        Self {
            work: 67_108_864,
            tiles: 4096,
            spans: 262_144,
        }
    }
}

#[derive(Debug)]
struct Dense {
    values: Vec<u8>,
    written: usize,
    _charge: BudgetAccount,
}
#[derive(Debug)]
enum Tile {
    Solid(u8),
    Dense(Dense),
}
impl Tile {
    fn at(&self, index: usize) -> u8 {
        match self {
            Self::Solid(value) => *value,
            Self::Dense(tile) => tile.values[index],
        }
    }
}
#[derive(Debug)]
struct Storage {
    tiles: HashMap<TileCoord, Tile>,
    bytes: u64,
    _charge: BudgetAccount,
}

/// Missing tiles equal the default inside `domain`; outside coverage is always zero.
/// Clones and inversion share immutable storage and its byte reservations. An
/// inversion flag changes logical R8 samples without copying every exception tile.
#[derive(Clone, Debug)]
pub struct BinaryMask {
    domain: Rect,
    default: u8,
    inverted: bool,
    offset: (i64, i64),
    storage: Option<Arc<Storage>>,
}
// Equality identifies the same immutable field view. Separately constructed fields
// use same_coverage_checked when a semantic no-op decision is required.
impl PartialEq for BinaryMask {
    fn eq(&self, other: &Self) -> bool {
        self.domain == other.domain
            && self.default == other.default
            && self.inverted == other.inverted
            && self.offset == other.offset
            && self.shares_storage(other)
    }
}
impl Eq for BinaryMask {}
impl BinaryMask {
    pub fn uniform(domain: Rect, selected: bool) -> Result<Self> {
        valid_domain(domain)?;
        Ok(Self {
            domain,
            default: if selected { 255 } else { 0 },
            inverted: false,
            offset: (0, 0),
            storage: None,
        })
    }
    /// Ordered, nonoverlapping runs use inclusive x1, like other coverage commands.
    /// Only 0 and 255 are accepted; fractional selection blending needs its own policy.
    pub fn from_spans(
        domain: Rect,
        selected_by_default: bool,
        spans: &[MaskSpan],
        budget: &SharedBudget,
        limits: MaskLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let mut build = builder::Builder::new(domain, selected_by_default, budget, limits)?;
        for span in spans {
            build.span(*span, check)?;
        }
        build.finish(check)
    }
    /// Rasterize a closed even-odd polygon at exact pixel centers, clipped to domain.
    /// Vertices are integer pixel-boundary coordinates; repeats and self-intersections
    /// follow the same even-odd rule, without floating-point intersections.
    pub fn polygon(
        domain: Rect,
        vertices: &[(i64, i64)],
        budget: &SharedBudget,
        limits: MaskLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        polygon::rasterize(domain, vertices, budget, limits, check)
    }
    /// Unique resident payload declaration. Views share this allocation identity;
    /// history may deduplicate it without treating mask storage as decoded RGBA.
    pub fn storage_usage(&self) -> Option<(usize, u64)> {
        self.storage
            .as_ref()
            .map(|storage| (Arc::as_ptr(storage) as usize, storage.bytes))
    }
    /// Exact bounded comparison, including the finite domain. Checks long rows
    /// cooperatively; a failed comparison cannot publish a selection change.
    pub fn same_coverage_checked(
        &self,
        other: &Self,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        check()?;
        if self == other {
            return Ok(true);
        }
        if self.domain != other.domain {
            return Ok(false);
        }
        if let (Some(a), Some(b)) = (self.uniform_coverage(), other.uniform_coverage()) {
            return Ok(a == b);
        }
        if self.domain.area() > u128::from(work) {
            return Err(limit());
        }
        for y in self.domain.y0..self.domain.y1 {
            for x in self.domain.x0..self.domain.x1 {
                if (x - self.domain.x0) % 1024 == 0 {
                    check()?;
                }
                if self.coverage(x, y) != other.coverage(x, y) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    pub fn domain(&self) -> Rect {
        self.domain
    }
    pub fn default_coverage(&self) -> u8 {
        self.logical(self.default)
    }
    pub fn stored_tiles(&self) -> usize {
        self.storage.as_ref().map_or(0, |s| s.tiles.len())
    }
    pub fn uniform_coverage(&self) -> Option<u8> {
        self.storage.is_none().then(|| self.default_coverage())
    }
    pub fn coverage(&self, x: i64, y: i64) -> u8 {
        if !self.domain.contains(x, y) {
            return 0;
        }
        let (x, y) = (x - self.offset.0, y - self.offset.1);
        let (tx, ix) = tile_split(x);
        let (ty, iy) = tile_split(y);
        let value = self
            .storage
            .as_ref()
            .and_then(|s| s.tiles.get(&TileCoord { tx, ty }))
            .map_or(self.default, |tile| {
                tile.at(iy as usize * crate::TILE_EDGE as usize + ix as usize)
            });
        self.logical(value)
    }
    fn logical(&self, value: u8) -> u8 {
        if self.inverted { 255 - value } else { value }
    }
    /// Complement strictly inside this finite domain; outside remains zero.
    pub fn inverted(&self) -> Self {
        let mut next = self.clone();
        next.inverted = !next.inverted;
        next
    }
    /// Clip without resampling or copying. None is an active empty intersection,
    /// never permission to edit the unrestricted target.
    pub fn clipped(&self, clip: Rect) -> Result<Option<Self>> {
        valid_domain(clip)?;
        Ok(self.domain.intersect(&clip).map(|domain| Self {
            domain,
            ..self.clone()
        }))
    }
    pub fn translated(&self, dx: i64, dy: i64) -> Result<Self> {
        let domain = self.domain.translated(dx, dy).ok_or_else(limit)?;
        valid_domain(domain).map_err(|_| limit())?;
        let offset = (
            self.offset.0.checked_add(dx).ok_or_else(limit)?,
            self.offset.1.checked_add(dy).ok_or_else(limit)?,
        );
        Ok(Self {
            domain,
            offset,
            ..self.clone()
        })
    }
    /// Storage identity only; inverted or translated views can have different fields.
    pub fn shares_storage(&self, other: &Self) -> bool {
        match (&self.storage, &other.storage) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
    /// Emit selected half-open ranges in ascending order, merging tile seams.
    /// Returns charged traversal work; callers can carry one remaining allowance
    /// across multiple rows. No row-sized temporary allocation is required.
    pub fn visit_row(
        &self,
        y: i64,
        x0: i64,
        x1: i64,
        work: u64,
        check: &dyn Fn() -> Result<()>,
        emit: impl FnMut(i64, i64) -> Result<()>,
    ) -> Result<u64> {
        rows::visit(self, y, x0, x1, work, check, emit)
    }
}
fn valid_domain(domain: Rect) -> Result<()> {
    if domain.x0 >= domain.x1
        || domain.y0 >= domain.y1
        || [domain.x0, domain.y0, domain.x1, domain.y1]
            .iter()
            .any(|n| n.unsigned_abs() > COORD_ABS_MAX as u64)
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid selection mask domain",
        ));
    }
    Ok(())
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "selection mask work or storage limit exceeded",
    )
}
fn schema(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn spend(work: &mut u64, amount: u64) -> Result<()> {
    *work = work.checked_sub(amount).ok_or_else(limit)?;
    Ok(())
}
