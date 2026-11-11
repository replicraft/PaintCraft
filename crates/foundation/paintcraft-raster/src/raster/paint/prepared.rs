//! Immutable shared replay program with one original control/payload admission.
use super::{
    Admission, BlendMode, Error, ErrorCode, PaintPlan, RasterLayer, Rect, Result, Rgba, TILE_EDGE,
    TileCoord, index, metadata,
};
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};
use std::{
    alloc::Layout,
    sync::{Arc, atomic::AtomicUsize},
};

/// Ordered paint retaining its original admitted immutable replay program.
/// Cloning shares the actual descriptor/index/control storage; it does not copy it.
#[derive(Clone, Debug)]
pub struct PreparedPaint(Arc<Program>);
#[derive(Debug)]
struct Program {
    index: index::Index,
    plan: PaintPlan,
    bounds: Rect,
    default: Rgba,
    _control: BudgetAccount,
    scope: BudgetScope,
}
impl PreparedPaint {
    pub(super) fn new(
        plan: PaintPlan,
        index: index::Index,
        bounds: Rect,
        default: Rgba,
        policy: (usize, BudgetScope),
    ) -> Result<Self> {
        let (limit, scope) = policy;
        // Current-toolchain ArcInner header plus the COMPLETE containing type,
        // including this account and original factory; payload Vec/map owners
        // retain their separate existing declarations. No allocator/RSS promise.
        let layout = Layout::new::<[AtomicUsize; 2]>()
            .extend(Layout::new::<Program>())
            .map_err(|_| metadata::exhausted())?
            .0
            .pad_to_align();
        metadata::Metadata::new(layout.size(), plan.metadata_limit.min(limit))?;
        let mut control = scope.account();
        control.resize(u64::try_from(layout.size()).map_err(|_| metadata::exhausted())?)?;
        Ok(Self(Arc::new(Program {
            index,
            plan,
            bounds,
            default,
            _control: control,
            scope,
        })))
    }
    /// Original immutable descriptors, without another payload copy or charge.
    pub fn plan(&self) -> &PaintPlan {
        &self.0.plan
    }
    /// Whether exact sparse/default rules produced indexed writes.
    pub fn is_empty(&self) -> bool {
        self.0.index.tiles.is_empty()
    }
    fn mismatch() -> Error {
        Error::new(
            ErrorCode::InvariantViolation,
            "compiled paint target differs from its simulated index",
        )
    }
    fn matches_target(
        &self,
        bounds: Rect,
        default: Rgba,
        target: (bool, impl Fn(TileCoord) -> bool),
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        check()?;
        let (empty, resident) = target;
        let program = &self.0;
        if bounds != program.bounds || default != program.default {
            return Ok(false);
        }
        let edge = i64::from(TILE_EDGE);
        // Same finite clipped traversal as index construction, preserving its
        // O(1) initial empty/default shortcut. Every tile observes cancellation.
        for (i, write) in program.plan.writes.iter().enumerate() {
            check()?;
            let Some(area) = write.area.intersect(&bounds) else {
                continue;
            };
            if empty
                && program.index.first_write.is_none_or(|first| first >= i)
                && write.mode == BlendMode::Replace
                && !write.destination_out
                && write.samples == write.sample_scale
                && write.color == default
            {
                if program.index.first_write == Some(i) {
                    return Ok(false);
                }
                continue;
            }
            for ty in area.y0 / edge..=(area.y1 - 1) / edge {
                for tx in area.x0 / edge..=(area.x1 - 1) / edge {
                    check()?;
                    let coord = TileCoord { tx, ty };
                    let writes = program.index.tiles.get(&coord);
                    let earlier = writes
                        .and_then(|values| values.first())
                        .is_some_and(|first| *first < i);
                    let expected = earlier || resident(coord) || write.pixel(default) != default;
                    let indexed = writes.is_some_and(|values| values.binary_search(&i).is_ok());
                    if expected != indexed {
                        return Ok(false);
                    }
                }
            }
        }
        check()?;
        Ok(true)
    }
    pub(super) fn reusable_checked(
        &self,
        plan: &PaintPlan,
        bounds: Rect,
        default: Rgba,
        policy: (usize, &BudgetScope),
        target: (bool, impl Fn(TileCoord) -> bool),
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        let limit = policy.0.min(plan.metadata_limit);
        if !self.0.scope.same_factory(policy.1)
            || self.0.index.bytes > limit
            || self.0._control.bytes() > u64::try_from(limit).map_err(|_| metadata::exhausted())?
            || self.plan().descriptor_bytes() > plan.descriptor_bytes()
            || !self.plan().same_writes_checked(plan, check)?
        {
            return Ok(false);
        }
        self.matches_target(bounds, default, target, check)
    }
    pub(super) fn coordinates(&self) -> impl Iterator<Item = TileCoord> + '_ {
        self.0.index.tiles.keys().copied()
    }
    /// Consume one handle, validate target, and borrow the original immutable index.
    /// No replay-index metadata allocation occurs here; shared handles remain valid.
    ///
    /// # Errors
    /// Rejects mismatch/cancellation/original decoded denial/allocator failure.
    /// Discard private staging after any error.
    pub fn replay_checked(
        self,
        layer: &mut RasterLayer,
        admission: &mut Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if !self.matches_target(
            layer.bounds(),
            layer.default,
            (layer.tiles.is_empty(), |coord| {
                layer.tiles.contains_key(&coord)
            }),
            check,
        )? {
            return Err(Self::mismatch());
        }
        let mut needed = 0u64;
        for coord in self.coordinates() {
            check()?;
            if !layer.private_tile(coord) {
                needed = needed.checked_add(1).ok_or_else(Self::mismatch)?;
            }
        }
        self.plan()
            .replay_index_checked(layer, admission, &self.0.index, needed, check)
    }
}
