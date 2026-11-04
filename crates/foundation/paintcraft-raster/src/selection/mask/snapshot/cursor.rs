use super::*;

#[derive(Debug)]
struct Emit<'a> {
    prefix: [u8; RECORD_BYTES],
    prefix_at: usize,
    payload_at: usize,
    tile: &'a Tile,
    rect: Rect,
    origin: (i64, i64),
}
/// Borrows immutable input; caller output remains caller-owned and caller-charged.
#[derive(Debug)]
pub struct SnapshotCursor<'a> {
    mask: &'a BinaryMask,
    callbacks: Callbacks<'a>,
    header: SnapshotHeader,
    grid: Option<Grid>,
    next: Option<(i64, i64)>,
    emit: Option<Emit<'a>>,
    remaining: u64,
    lookup_headroom: u64,
    failed: bool,
    _charge: BudgetAccount,
}
impl BinaryMask {
    pub fn snapshot_quote(
        &self,
        layer_extent: Rect,
        limits: SnapshotLimits,
    ) -> Result<SnapshotQuote> {
        valid_domain(layer_extent)?;
        let clip = self.domain.intersect(&layer_extent);
        let active = clip.zip(self.storage.as_ref());
        let (cells, records, samples, headroom) = if let Some((clip, storage)) = active {
            // Provenance: only Builder inserts into this private immutable map.
            let layout = map_layout(storage.tiles.len())?;
            if layout.capacity != storage.tiles.capacity() {
                return Err(limit());
            }
            let cells = grid(clip, self.offset)?.cells;
            let records = cells.min(storage.tiles.len() as u64);
            // Distinct backing coordinates identify disjoint translated tiles.
            // Their clipped union has area <= clip.area(), and every emitted
            // tile contains <= 65,536 samples. Include Solid in this bound;
            // no counting/sample pass is needed. Min in u128 before converting
            // so a huge sparse clip need not have a u64-representable area.
            let samples =
                u64::try_from(clip.area().min(u128::from(records) * DENSE_SAMPLES as u128))
                    .map_err(|_| limit())?;
            (
                cells,
                records,
                samples,
                lookup_headroom(storage.tiles.len())?.max(GRID_WORK),
            )
        } else {
            (0, 0, 0, HEADER_WORK)
        };
        let weighted = self.storage.as_ref().is_some_and(|s| {
            s.tiles.values().any(|t| match t {
                Tile::Solid(v) => !matches!(*v, 0 | 255),
                Tile::Dense(d) => d.weighted,
            })
        });
        let payload = if weighted {
            samples
        } else {
            add(samples, mul(records, 7)?)? / 8
        };
        let bytes = add(
            HEADER_BYTES as u64,
            add(mul(records, RECORD_BYTES as u64)?, payload)?,
        )?;
        let work = add(
            HEADER_WORK,
            add(
                mul(cells, GRID_WORK + RECORD_WORK)?,
                add(
                    mul(samples, SAMPLE_WORK)?,
                    mul(bytes - HEADER_BYTES as u64, BYTE_WORK)?,
                )?,
            )?,
        )?;
        if cells > limits.grid_cells
            || records > u64::from(limits.records)
            || bytes > limits.bytes
            || work > limits.work
            || headroom > limits.indivisible_work
        {
            return Err(limit());
        }
        Ok(SnapshotQuote {
            bytes_upper: bytes,
            control_bytes: size_of::<SnapshotCursor<'_>>() as u64,
            work_upper: work,
            grid_cells: cells,
            indivisible_work: headroom.max(HEADER_WORK),
        })
    }
    pub fn snapshot_cursor<'a>(
        &'a self,
        layer_extent: Rect,
        scope: &BudgetScope,
        limits: SnapshotLimits,
        check: &'a dyn Fn() -> Result<()>,
        spend: &'a dyn Fn(u64) -> Result<()>,
    ) -> Result<SnapshotCursor<'a>> {
        check()?;
        let quote = self.snapshot_quote(layer_extent, limits)?;
        spend(HEADER_WORK)?;
        let clip = self.domain.intersect(&layer_extent);
        let grid = if self.storage.is_some() {
            clip.map(|clip| grid(clip, self.offset)).transpose()?
        } else {
            None
        };
        let mut charge = scope.account();
        charge.resize(quote.control_bytes)?;
        Ok(SnapshotCursor {
            mask: self,
            callbacks: Callbacks { check, spend },
            header: SnapshotHeader {
                layer_extent,
                clip,
                offset: self.offset,
                default: self.default,
                inverted: self.inverted,
                records: 0,
                total_bytes: HEADER_BYTES as u64,
            },
            next: grid.map(|g| (g.x0, g.y0)),
            grid,
            emit: None,
            remaining: limits.work - HEADER_WORK,
            lookup_headroom: quote.indivisible_work,
            failed: false,
            _charge: charge,
        })
    }
}
impl<'a> SnapshotCursor<'a> {
    /// Each successful chunk is private until finish_header succeeds. On error
    /// discard all prior chunks; a failed cursor cannot be resumed or sealed.
    pub fn step(&mut self, output: &mut [u8], quantum: u64) -> Result<SnapshotProgress> {
        if self.failed {
            return Err(bad("failed selection snapshot cursor"));
        }
        let result = self.step_inner(output, quantum);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn step_inner(&mut self, output: &mut [u8], quantum: u64) -> Result<SnapshotProgress> {
        let Callbacks { check, spend } = self.callbacks;
        check()?;
        if output.is_empty() {
            return Err(bad("selection snapshot empty output chunk"));
        }
        let mut meter = Meter {
            quantum,
            work: &mut self.remaining,
            check,
            spend,
        };
        let mut written = 0;
        while written < output.len() {
            if let Some(emit) = &mut self.emit {
                if emit.prefix_at < RECORD_BYTES {
                    if !meter.take(BYTE_WORK, 0)? {
                        break;
                    }
                    output[written] = emit.prefix[emit.prefix_at];
                    emit.prefix_at += 1;
                    written += 1;
                    continue;
                }
                let payload = u32_at(&emit.prefix, 24) as usize;
                if emit.payload_at == payload {
                    self.emit = None;
                    continue;
                }
                let width = (emit.rect.x1 - emit.rect.x0) as usize;
                let samples = width * (emit.rect.y1 - emit.rect.y0) as usize;
                if emit.prefix[16] == 2 {
                    if !meter.take(BYTE_WORK + SAMPLE_WORK, 0)? {
                        break;
                    }
                    let index = emit.payload_at;
                    let x = emit.rect.x0 + (index % width) as i64 - emit.origin.0;
                    let y = emit.rect.y0 + (index / width) as i64 - emit.origin.1;
                    output[written] = emit.tile.at(y as usize * 256 + x as usize);
                    emit.payload_at += 1;
                    written += 1;
                    continue;
                }
                let at = emit.payload_at * 8;
                let count = 8.min(samples - at);
                if !meter.take(BYTE_WORK + count as u64 * SAMPLE_WORK, 0)? {
                    break;
                }
                let mut value = 0;
                for bit in 0..count {
                    let index = at + bit;
                    let x = emit.rect.x0 + (index % width) as i64 - emit.origin.0;
                    let y = emit.rect.y0 + (index / width) as i64 - emit.origin.1;
                    let sample = emit.tile.at(y as usize * 256 + x as usize);
                    if !matches!(sample, 0 | 255) {
                        return Err(bad("nonbinary selection backing"));
                    }
                    value |= (sample / 255) << bit;
                }
                output[written] = value;
                emit.payload_at += 1;
                written += 1;
                continue;
            }
            let Some((tx, ty)) = self.next else { break };
            if !meter.take(GRID_WORK + RECORD_WORK, self.lookup_headroom)? {
                break;
            }
            let grid = self.grid.expect("next implies grid");
            self.next = if tx < grid.x1 {
                Some((tx + 1, ty))
            } else if ty < grid.y1 {
                Some((grid.x0, ty + 1))
            } else {
                None
            };
            let mask: &'a BinaryMask = self.mask;
            let storage = mask.storage.as_ref().expect("grid implies storage");
            let Some(tile) = storage.tiles.get(&TileCoord { tx, ty }) else {
                continue;
            };
            let rect = tile_clip(
                tx,
                ty,
                self.header.offset,
                self.header.clip.expect("grid clip"),
            )?;
            let mut prefix = [0; RECORD_BYTES];
            prefix[..8].copy_from_slice(&tx.to_be_bytes());
            prefix[8..16].copy_from_slice(&ty.to_be_bytes());
            let samples = (rect.x1 - rect.x0) as usize * (rect.y1 - rect.y0) as usize;
            let payload = match tile {
                Tile::Solid(value) => {
                    prefix[17] = *value;
                    0
                }
                Tile::Dense(dense) => {
                    prefix[16] = if dense.weighted { 2 } else { 1 };
                    if dense.weighted {
                        samples
                    } else {
                        samples.div_ceil(8)
                    }
                }
            };
            prefix[20..22].copy_from_slice(&((rect.x1 - rect.x0) as u16).to_be_bytes());
            prefix[22..24].copy_from_slice(&((rect.y1 - rect.y0) as u16).to_be_bytes());
            prefix[24..28].copy_from_slice(&(payload as u32).to_be_bytes());
            self.header.records = self.header.records.checked_add(1).ok_or_else(limit)?;
            self.emit = Some(Emit {
                prefix,
                prefix_at: 0,
                payload_at: 0,
                tile,
                rect,
                origin: (
                    tx.checked_mul(256)
                        .and_then(|x| x.checked_add(self.header.offset.0))
                        .ok_or_else(limit)?,
                    ty.checked_mul(256)
                        .and_then(|y| y.checked_add(self.header.offset.1))
                        .ok_or_else(limit)?,
                ),
            });
        }
        self.header.total_bytes = add(self.header.total_bytes, written as u64)?;
        // An exactly filled final prefix/payload needs no extra caller roundtrip.
        if self.emit.as_ref().is_some_and(|e| {
            e.prefix_at == RECORD_BYTES && e.payload_at == u32_at(&e.prefix, 24) as usize
        }) {
            self.emit = None;
        }
        check()?;
        if self.next.is_none() && self.emit.is_none() {
            Ok(SnapshotProgress::Complete { written })
        } else {
            Ok(SnapshotProgress::Pending { written })
        }
    }
    pub fn finish_header(&mut self) -> Result<[u8; HEADER_BYTES]> {
        if self.failed || self.next.is_some() || self.emit.is_some() {
            return Err(bad("selection snapshot incomplete"));
        }
        if let Err(error) = (self.callbacks.check)() {
            self.failed = true;
            return Err(error);
        }
        Ok(self.header.encode())
    }
}
