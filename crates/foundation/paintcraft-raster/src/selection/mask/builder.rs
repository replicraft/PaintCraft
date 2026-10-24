//! Private monotonic span construction; allocations drop before their accounts.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetScope};
use std::mem::size_of;
const EDGE: usize = crate::TILE_EDGE as usize;
const PIXELS: usize = EDGE * EDGE;

pub(super) struct Builder {
    domain: Rect,
    default: u8,
    scope: BudgetScope,
    tiles: HashMap<TileCoord, Tile>,
    charge: BudgetAccount,
    limits: MaskLimits,
    remaining: u64,
    spans: usize,
    previous: Option<(i64, i64)>,
}
impl Builder {
    pub(super) fn new(
        domain: Rect,
        default: bool,
        budget: &SharedBudget,
        limits: MaskLimits,
    ) -> Result<Self> {
        valid_domain(domain)?;
        let scope = budget.scope(AllocationClass::SelectionMasks);
        let charge = scope.account();
        Ok(Self {
            domain,
            default: if default { 255 } else { 0 },
            tiles: HashMap::new(),
            charge,
            scope,
            limits,
            remaining: limits.work,
            spans: 0,
            previous: None,
        })
    }
    pub(super) fn account(&self) -> BudgetAccount {
        self.scope.account()
    }
    pub(super) fn spend(&mut self, amount: u64) -> Result<()> {
        spend(&mut self.remaining, amount)
    }
    pub(super) fn remaining_work(&self) -> u64 {
        self.remaining
    }
    pub(super) fn retained_tiles(&self) -> usize {
        self.tiles.len()
    }
    pub(super) fn span(&mut self, span: MaskSpan, check: &dyn Fn() -> Result<()>) -> Result<()> {
        check()?;
        self.spend(1)?;
        if self.spans == self.limits.spans {
            return Err(limit());
        }
        if span.x0 > span.x1
            || !self.domain.contains(span.x0, span.y)
            || !self.domain.contains(span.x1, span.y)
            || !matches!(span.coverage, 0 | 255)
            || self
                .previous
                .is_some_and(|(y, x)| span.y < y || (span.y == y && span.x0 <= x))
        {
            return Err(schema(
                "selection runs must be ordered, disjoint, binary and inside their domain",
            ));
        }
        self.spans += 1;
        self.previous = Some((span.y, span.x1));
        if span.coverage == self.default {
            return Ok(());
        }
        let (ty, iy) = tile_split(span.y);
        let mut x = span.x0;
        while x <= span.x1 {
            check()?;
            let (tx, ix) = tile_split(x);
            let end = span.x1.min((tx + 1) * EDGE as i64 - 1);
            let length = (end - x + 1) as usize;
            self.spend(length as u64)?;
            let coord = TileCoord { tx, ty };
            if !self.tiles.contains_key(&coord) {
                self.reserve_entry()?;
                let tile = Tile::Dense(dense(self.default, self.account())?);
                self.tiles.insert(coord, tile);
            }
            let tile = self.tiles.get_mut(&coord).expect("inserted mask tile");
            let Tile::Dense(dense) = tile else {
                return Err(schema("overlapping mask tile write"));
            };
            let start = iy as usize * EDGE + ix as usize;
            dense.values[start..start + length].fill(span.coverage);
            dense.written += length;
            let tile_area = Rect::new(tx * EDGE as i64, ty * EDGE as i64, EDGE as i64, EDGE as i64)
                .unwrap()
                .intersect(&self.domain)
                .expect("validated span is inside the tile")
                .area();
            if dense.written as u128 == tile_area {
                *tile = Tile::Solid(span.coverage);
            }
            x = end + 1;
        }
        Ok(())
    }
    fn reserve_entry(&mut self) -> Result<()> {
        if self.tiles.len() == self.limits.tiles {
            return Err(limit());
        }
        if self.tiles.len() < self.tiles.capacity() {
            return Ok(());
        }
        let before = self.charge.bytes();
        let width = size_of::<(TileCoord, Tile)>() as u64;
        self.charge.resize(if before == 0 {
            size_of::<Storage>() as u64 + width
        } else {
            before.checked_add(width).ok_or_else(limit)?
        })?;
        self.tiles.try_reserve(1).map_err(|_| limit())?;
        let bytes = (self.tiles.capacity() as u64)
            .checked_mul(width)
            .and_then(|n| n.checked_add(size_of::<Storage>() as u64))
            .ok_or_else(limit)?;
        if let Err(error) = self.charge.resize(bytes) {
            // Reallocation can round capacity upward. Drop uncharged storage
            // before releasing the account or returning its denial.
            drop(std::mem::take(&mut self.tiles));
            self.charge.resize(0)?;
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn finish(mut self, check: &dyn Fn() -> Result<()>) -> Result<BinaryMask> {
        check()?;
        let count = (self.domain.x1 - 1).div_euclid(EDGE as i64)
            - self.domain.x0.div_euclid(EDGE as i64)
            + 1;
        let rows = (self.domain.y1 - 1).div_euclid(EDGE as i64)
            - self.domain.y0.div_euclid(EDGE as i64)
            + 1;
        let mut full = self.tiles.len() as u128 == count as u128 * rows as u128;
        for tile in self.tiles.values() {
            check()?;
            spend(&mut self.remaining, 1)?;
            full &= matches!(tile,Tile::Solid(value) if *value != self.default);
        }
        if full {
            return BinaryMask::uniform(self.domain, self.default == 0);
        }
        let storage = (!self.tiles.is_empty()).then(|| {
            let bytes = self.charge.bytes()
                + self
                    .tiles
                    .values()
                    .map(|tile| match tile {
                        Tile::Dense(dense) => dense._charge.bytes(),
                        Tile::Solid(_) => 0,
                    })
                    .sum::<u64>();
            Arc::new(Storage {
                bytes,
                tiles: self.tiles,
                _charge: self.charge,
            })
        });
        Ok(BinaryMask {
            domain: self.domain,
            default: self.default,
            inverted: false,
            offset: (0, 0),
            storage,
        })
    }
}
fn dense(default: u8, mut charge: BudgetAccount) -> Result<Dense> {
    charge.resize(PIXELS as u64)?;
    let mut values = Vec::new();
    values.try_reserve_exact(PIXELS).map_err(|_| limit())?;
    if let Err(error) = charge.resize(values.capacity() as u64) {
        drop(values);
        return Err(error);
    }
    values.resize(PIXELS, default);
    Ok(Dense {
        values,
        written: 0,
        _charge: charge,
    })
}
