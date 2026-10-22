//! Source-bound logical pixels and branchable snapshots, without decoded replay.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use std::mem::size_of;

enum Data {
    Raster(RasterLayer),
    Program(PaintProjection),
}
struct Node {
    bounds: Rect,
    default: Rgba,
    data: Data,
    parent: Option<Arc<Node>>,
    // Reachable payload, counting shared ancestors once on this lineage only.
    bytes: usize,
    _charge: BudgetAccount,
}
impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaintSource")
            .field("bounds", &self.bounds)
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        // Branches can form long singly owned tails. Release those iteratively,
        // stopping at shared ancestors; ordinary recursive Arc drop can overflow.
        let mut tail = self.parent.take();
        while let Some(parent) = tail {
            // into_inner consumes a losing reference without dropping an Err Arc
            // that could become the last owner concurrently and recurse here.
            match Arc::into_inner(parent) {
                Some(mut node) => tail = node.parent.take(),
                None => break,
            }
        }
    }
}

/// Immutable captured source and logical paint. Clones share source ownership and
/// metadata; no decoded pixels are allocated. Captured decoded inputs stay leased.
#[derive(Clone, Debug)]
pub struct PaintSnapshot {
    source: Arc<Node>,
    bounds: Rect,
    default: Rgba,
    has_tiles: bool,
}
/// One mutable logical branch. Append consumes the branch on failure; snapshots
/// taken before it stay immutable. This is not a transaction/publication API.
#[derive(Debug)]
pub struct PaintOverlay {
    source: PaintSnapshot,
    program: PaintProjection,
    bounds: Rect,
}
/// A bounded, fallibly prepared lineage walk. Repeated pixel reads allocate no
/// scratch and never query a mutable registry or caller-supplied replacement source.
#[derive(Debug)]
pub struct PaintReader<'a> {
    source: &'a PaintSnapshot,
    program: Option<&'a PaintProjection>,
    bounds: Rect,
    chain: Vec<&'a Node>,
    charge: BudgetAccount,
}
fn node_charge(admission: &Admission, extra: usize) -> Result<BudgetAccount> {
    let bytes = size_of::<Node>()
        .checked_add(extra)
        .ok_or_else(metadata::exhausted)?;
    metadata::Metadata::new(bytes, admission.paint_metadata_limit)?;
    let mut charge = admission.paint_metadata_scope.account();
    charge.resize(bytes as u64)?;
    Ok(charge)
}
impl PaintSnapshot {
    /// Capture an owned sparse raster and its immutable decoded leases. Metadata
    /// payload includes the node and usable source-map capacity, excluding allocator
    /// and Arc controls. It does not copy or allocate pixel buffers.
    pub fn new(
        source: RasterLayer,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if source.width == 0 || source.height == 0 {
            return Err(Error::new(ErrorCode::InputSchema, "empty paint source"));
        }
        let extra = source
            .tiles
            .capacity()
            .checked_mul(size_of::<(TileCoord, Arc<Tile>)>())
            .ok_or_else(metadata::exhausted)?;
        let charge = node_charge(admission, extra)?;
        let bounds = source.bounds();
        let default = source.default;
        let has_tiles = !source.tiles.is_empty();
        let bytes = charge.bytes() as usize;
        let snapshot = Self {
            source: Arc::new(Node {
                bounds,
                default,
                data: Data::Raster(source),
                parent: None,
                bytes,
                _charge: charge,
            }),
            bounds,
            default,
            has_tiles,
        };
        check()?;
        Ok(snapshot)
    }
    pub fn bounds(&self) -> Rect {
        self.bounds
    }
    /// Reachable declared payload. Shared ancestors must not be added repeatedly
    /// when totaling several snapshots; the shared budget owns actual declarations.
    pub fn metadata_bytes(&self) -> usize {
        self.source.bytes
    }
    pub fn fork(
        &self,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintOverlay> {
        self.fork_expanded(
            self.bounds.width() as u32,
            self.bounds.height() as u32,
            admission,
            check,
        )
    }
    /// Expansion preserves the old source's clipping and reveals its implicit
    /// default outside the old bounds, including padding in partial edge tiles.
    pub fn fork_expanded(
        &self,
        width: u32,
        height: u32,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintOverlay> {
        check()?;
        let mut extent = RasterLayer::new(
            self.bounds.width() as u32,
            self.bounds.height() as u32,
            self.default,
        );
        extent.expand_to(width, height)?;
        let bounds = extent.bounds();
        Ok(PaintOverlay {
            source: self.clone(),
            program: admission
                .paint_plan()
                .into_projection_checked(bounds, check)?,
            bounds,
        })
    }
    pub fn reader<'a>(
        &'a self,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintReader<'a>> {
        PaintReader::new(self, None, self.bounds, admission, check)
    }
    fn has_tile(&self, coord: TileCoord, check: &dyn Fn() -> Result<()>) -> Result<bool> {
        if !self.has_tiles {
            return Ok(false);
        }
        let mut node = Some(self.source.as_ref());
        while let Some(current) = node {
            check()?;
            let present = match &current.data {
                Data::Raster(raster) => raster.tiles.contains_key(&coord),
                Data::Program(program) => program.contains_tile(coord),
            };
            if present {
                return Ok(true);
            }
            node = current.parent.as_deref();
        }
        Ok(false)
    }
}
struct SourceTarget<'a> {
    source: &'a PaintSnapshot,
    bounds: Rect,
    check: &'a dyn Fn() -> Result<()>,
}
impl index::Target for SourceTarget<'_> {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn skip_empty(&self, write: Write) -> bool {
        !self.source.has_tiles && write.pixel(self.source.default) == self.source.default
    }
    fn skip_missing(&self, coord: TileCoord, write: Write) -> Result<bool> {
        Ok(write.pixel(self.source.default) == self.source.default
            && !self.source.has_tile(coord, self.check)?)
    }
    fn admit_tile(&mut self, _: TileCoord) -> Result<()> {
        Ok(())
    }
}
impl PaintOverlay {
    /// Source-bound no-op proofs may discard descriptors. The resulting program
    /// cannot be detached and applied to an unrelated source.
    pub fn append_checked(
        self,
        incoming: PaintPlan,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let uniform = !self.source.has_tiles && !self.program.has_tiles();
        let mut noop = true;
        for write in incoming.writes.iter() {
            check()?;
            if write.area.intersect(&self.bounds).is_some()
                && !(uniform && write.pixel(self.source.default) == self.source.default)
            {
                noop = false;
                break;
            }
        }
        if noop {
            return Ok(self);
        }
        let program = self.program.append_target_checked(
            incoming,
            &mut SourceTarget {
                source: &self.source,
                bounds: self.bounds,
                check,
            },
            check,
        )?;
        Ok(Self {
            source: self.source,
            program,
            bounds: self.bounds,
        })
    }
    pub fn metadata_bytes(&self) -> usize {
        self.source.metadata_bytes() + self.program.metadata_bytes()
    }
    pub fn reader<'a>(
        &'a self,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintReader<'a>> {
        PaintReader::new(
            &self.source,
            Some(&self.program),
            self.bounds,
            admission,
            check,
        )
    }
    pub fn freeze_checked(
        self,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintSnapshot> {
        check()?;
        if !self.program.has_tiles() {
            return Ok(PaintSnapshot {
                bounds: self.bounds,
                ..self.source
            });
        }
        let charge = node_charge(admission, 0)?;
        let bytes = self
            .source
            .metadata_bytes()
            .checked_add(self.program.metadata_bytes())
            .and_then(|n| n.checked_add(charge.bytes() as usize))
            .ok_or_else(metadata::exhausted)?;
        Ok(PaintSnapshot {
            source: Arc::new(Node {
                bounds: self.bounds,
                default: self.source.default,
                data: Data::Program(self.program),
                parent: Some(self.source.source),
                bytes,
                _charge: charge,
            }),
            bounds: self.bounds,
            default: self.source.default,
            has_tiles: true,
        })
    }
}
impl<'a> PaintReader<'a> {
    fn new(
        source: &'a PaintSnapshot,
        program: Option<&'a PaintProjection>,
        bounds: Rect,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut charge = admission.paint_metadata_scope.account();
        let mut chain = Vec::new();
        check()?;
        // Only this source-bound type can prove that all logical pixels retain
        // the captured implicit default. Empty chain encodes that exact proof.
        if !source.has_tiles && program.is_none_or(|p| !p.has_tiles()) {
            return Ok(Self {
                source,
                program,
                bounds,
                chain,
                charge,
            });
        }
        let mut metadata = metadata::Metadata::new(0, admission.paint_metadata_limit)?;
        let mut node = Some(source.source.as_ref());
        while let Some(current) = node {
            check()?;
            metadata.vector(&mut chain, &mut charge)?;
            chain.push(current);
            node = current.parent.as_deref();
        }
        check()?;
        Ok(Self {
            source,
            program,
            bounds,
            chain,
            charge,
        })
    }
    pub fn scratch_bytes(&self) -> u64 {
        self.charge.bytes()
    }
    /// Outside the captured extent returns its implicit default, as RasterLayer::get.
    pub fn read_checked(&self, x: i64, y: i64, check: &dyn Fn() -> Result<()>) -> Result<Rgba> {
        check()?;
        if self.chain.is_empty() || !self.bounds.contains(x, y) {
            return Ok(self.source.default);
        }
        let mut pixel = self.source.default;
        for node in self.chain.iter().rev() {
            check()?;
            if !node.bounds.contains(x, y) {
                pixel = node.default;
                continue;
            }
            pixel = match &node.data {
                Data::Raster(raster) => raster.get(x, y),
                Data::Program(program) => program.read_checked(x, y, |_, _| Ok(pixel), check)?,
            };
        }
        if let Some(program) = self.program {
            pixel = program.read_checked(x, y, |_, _| Ok(pixel), check)?;
        }
        Ok(pixel)
    }
}
