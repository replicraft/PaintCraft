use super::*;

#[derive(Debug)]
pub enum CapturedSelection {
    Empty,
    Field(BinaryMask),
}
#[derive(Debug)]
pub struct DecodedSnapshot {
    pub header: SnapshotHeader,
    pub selection: CapturedSelection,
}
#[derive(Clone, Copy, Debug)]
pub struct DecodeQuote {
    pub control_bytes: u64,
    pub retained_upper: u64,
    pub allocation_peak: u64,
    pub work_upper: u64,
    pub indivisible_work: u64,
}
#[derive(Clone, Copy, Debug)]
struct Record {
    coordinate: TileCoord,
    rect: Rect,
    kind: u8,
    value: u8,
    payload: usize,
    next: usize,
}
impl Record {
    fn parse(bytes: &[u8], at: usize, header: SnapshotHeader) -> Result<Self> {
        let end = at.checked_add(RECORD_BYTES).ok_or_else(limit)?;
        let part = bytes
            .get(at..end)
            .ok_or_else(|| bad("truncated selection record"))?;
        if part[16] > 2
            || part[18..20].iter().chain(&part[28..32]).any(|v| *v != 0)
            || (part[16] != 0 && part[17] != 0)
        {
            return Err(bad("selection record flags"));
        }
        let coordinate = TileCoord {
            tx: i64_at(part, 0),
            ty: i64_at(part, 8),
        };
        let rect = tile_clip(
            coordinate.tx,
            coordinate.ty,
            header.offset,
            header
                .clip
                .ok_or_else(|| bad("record in empty selection"))?,
        )?;
        let width = u16::from_be_bytes(part[20..22].try_into().expect("fixed width"));
        let height = u16::from_be_bytes(part[22..24].try_into().expect("fixed height"));
        let samples = (rect.x1 - rect.x0) as usize * (rect.y1 - rect.y0) as usize;
        let expected = if part[16] == 0 {
            0
        } else if part[16] == 2 {
            samples
        } else {
            samples.div_ceil(8)
        };
        if i64::from(width) != rect.x1 - rect.x0
            || i64::from(height) != rect.y1 - rect.y0
            || u32_at(part, 24) as usize != expected
        {
            return Err(bad("selection record dimensions/length"));
        }
        let next = end.checked_add(expected).ok_or_else(limit)?;
        let payload = bytes
            .get(end..next)
            .ok_or_else(|| bad("truncated selection payload"))?;
        if part[16] == 1
            && !samples.is_multiple_of(8)
            && payload.last().is_some_and(|v| *v >> (samples % 8) != 0)
        {
            return Err(bad("selection record nonzero padding"));
        }
        Ok(Self {
            coordinate,
            rect,
            kind: part[16],
            value: part[17],
            payload: end,
            next,
        })
    }
    fn samples(self) -> usize {
        (self.rect.x1 - self.rect.x0) as usize * (self.rect.y1 - self.rect.y0) as usize
    }
}
#[derive(Debug)]
struct PendingDense {
    record: Record,
    dense: Dense,
    at: usize,
}
/// Two resumable passes: validate/quote without allocations, then scope-backed
/// construction. Any error poisons the decoder; all partial storage stays private.
#[derive(Debug)]
pub struct SnapshotDecoder<'a> {
    bytes: &'a [u8],
    callbacks: Callbacks<'a>,
    header: SnapshotHeader,
    limits: SnapshotLimits,
    remaining: u64,
    scope: BudgetScope,
    scan_at: usize,
    scan_count: u32,
    previous: Option<(i64, i64)>,
    dense_count: u64,
    samples: u64,
    quote: Option<DecodeQuote>,
    build_at: usize,
    build_count: u32,
    building: Option<Record>,
    pending: Option<PendingDense>,
    tiles: HashMap<TileCoord, Tile>,
    failed: bool,
    done: bool,
    _control: BudgetAccount,
    map_charge: BudgetAccount,
}
impl<'a> SnapshotDecoder<'a> {
    pub fn new(
        bytes: &'a [u8],
        scope: &BudgetScope,
        limits: SnapshotLimits,
        check: &'a dyn Fn() -> Result<()>,
        spend: &'a dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if limits.work < HEADER_WORK || limits.indivisible_work < HEADER_WORK {
            return Err(limit());
        }
        spend(HEADER_WORK)?;
        let header = SnapshotHeader::parse(bytes, limits)?;
        // Zero records require no grid/offset arithmetic, even for huge uniform fields.
        if header.records != 0 {
            let cells = grid(
                header.clip.ok_or_else(|| bad("empty record clip"))?,
                header.offset,
            )?
            .cells;
            if cells > limits.grid_cells || u64::from(header.records) > cells {
                return Err(limit());
            }
        }
        let mut control = scope.account();
        control.resize(size_of::<Self>() as u64)?;
        Ok(Self {
            bytes,
            callbacks: Callbacks { check, spend },
            header,
            limits,
            remaining: limits.work - HEADER_WORK,
            scope: scope.clone(),
            scan_at: HEADER_BYTES,
            scan_count: 0,
            previous: None,
            dense_count: 0,
            samples: 0,
            quote: None,
            build_at: HEADER_BYTES,
            build_count: 0,
            building: None,
            pending: None,
            tiles: HashMap::new(),
            failed: false,
            done: false,
            _control: control,
            map_charge: scope.account(),
        })
    }
    pub fn header(&self) -> SnapshotHeader {
        self.header
    }
    /// Available after the prefix pass, before any retained backing allocation.
    pub fn quote(&self) -> Option<DecodeQuote> {
        self.quote
    }
    pub fn step(&mut self, quantum: u64) -> Result<SnapshotProgress> {
        if self.failed {
            return Err(bad("failed selection snapshot decoder"));
        }
        let result = self.step_inner(quantum);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn step_inner(&mut self, quantum: u64) -> Result<SnapshotProgress> {
        let Callbacks { check, spend } = self.callbacks;
        check()?;
        let mut meter = Meter {
            quantum,
            work: &mut self.remaining,
            check,
            spend,
        };
        while self.scan_count < self.header.records {
            if !meter.take(RECORD_WORK, RECORD_WORK)? {
                return Ok(SnapshotProgress::Pending { written: 0 });
            }
            let record = Record::parse(self.bytes, self.scan_at, self.header)?;
            let order = (record.coordinate.ty, record.coordinate.tx);
            if self.previous.is_some_and(|previous| previous >= order) {
                return Err(bad("unordered/duplicate selection record"));
            }
            self.previous = Some(order);
            self.scan_count += 1;
            self.scan_at = record.next;
            if record.kind != 0 {
                self.dense_count = add(self.dense_count, 1)?;
                self.samples = add(self.samples, record.samples() as u64)?;
            }
        }
        if self.quote.is_none() {
            if self.scan_at != self.bytes.len() {
                return Err(bad("trailing selection snapshot bytes"));
            }
            let records = self.header.records as usize;
            let layout = map_layout(records)?;
            let map_bytes = if records == 0 {
                0
            } else {
                add(
                    layout.bytes,
                    (size_of::<Storage>() + 2 * size_of::<usize>()) as u64,
                )?
            };
            // Covers old+new allocation overlap at each minimal-growth reserve.
            let map_peak = mul(map_bytes, 2)?;
            let dense_bytes = mul(self.dense_count, DENSE_SAMPLES as u64)?;
            let work = add(
                HEADER_WORK,
                add(
                    mul(u64::from(self.header.records), RECORD_WORK * 3)?,
                    add(
                        mul(self.dense_count, DENSE_INIT_WORK)?,
                        mul(self.samples, SAMPLE_WORK)?,
                    )?,
                )?,
            )?;
            let headroom = lookup_headroom(records)?
                .max(if self.dense_count == 0 {
                    RECORD_WORK
                } else {
                    DENSE_INIT_WORK
                })
                .max(HEADER_WORK);
            if work > self.limits.work || headroom > self.limits.indivisible_work {
                return Err(limit());
            }
            self.quote = Some(DecodeQuote {
                control_bytes: size_of::<Self>() as u64,
                retained_upper: add(map_bytes, dense_bytes)?,
                allocation_peak: add(size_of::<Self>() as u64, add(map_peak, dense_bytes)?)?,
                work_upper: work,
                indivisible_work: headroom,
            });
            // Admit the complete private backing upper before its first allocation.
            // Dense accounts subsequently take portions from this same declaration.
            self.map_charge.resize(add(map_peak, dense_bytes)?)?;
        }
        loop {
            if let Some(pending) = &mut self.pending {
                let count = 64.min(pending.record.samples() - pending.at);
                if count != 0 {
                    if !meter.take(count as u64 * SAMPLE_WORK, 0)? {
                        return Ok(SnapshotProgress::Pending { written: 0 });
                    }
                    let width = (pending.record.rect.x1 - pending.record.rect.x0) as usize;
                    let origin = |t: i64, o: i64| {
                        t.checked_mul(256)
                            .and_then(|v| v.checked_add(o))
                            .ok_or_else(limit)
                    };
                    let ox = origin(pending.record.coordinate.tx, self.header.offset.0)?;
                    let oy = origin(pending.record.coordinate.ty, self.header.offset.1)?;
                    for i in pending.at..pending.at + count {
                        let value = if pending.record.kind == 2 {
                            self.bytes[pending.record.payload + i]
                        } else {
                            ((self.bytes[pending.record.payload + i / 8] >> (i % 8)) & 1) * 255
                        };
                        let x = pending.record.rect.x0 + (i % width) as i64 - ox;
                        let y = pending.record.rect.y0 + (i / width) as i64 - oy;
                        pending.dense.values[y as usize * 256 + x as usize] = value;
                    }
                    pending.at += count;
                    continue;
                }
                if !meter.take(RECORD_WORK, lookup_headroom(self.header.records as usize)?)? {
                    return Ok(SnapshotProgress::Pending { written: 0 });
                }
                let pending = self.pending.take().expect("checked pending");
                insert(
                    &mut self.tiles,
                    pending.record.coordinate,
                    Tile::Dense(pending.dense),
                )?;
                self.build_at = pending.record.next;
                self.build_count += 1;
            }
            if self.build_count == self.header.records {
                self.done = true;
                check()?;
                return Ok(SnapshotProgress::Complete { written: 0 });
            }
            if self.building.is_none() {
                if !meter.take(RECORD_WORK, 0)? {
                    return Ok(SnapshotProgress::Pending { written: 0 });
                }
                self.building = Some(Record::parse(self.bytes, self.build_at, self.header)?);
            }
            let record = self.building.expect("charged parsed build record");
            let work = if record.kind != 0 {
                DENSE_INIT_WORK
            } else {
                RECORD_WORK
            };
            let headroom = lookup_headroom(self.header.records as usize)?.max(work);
            if !meter.take(work, headroom)? {
                return Ok(SnapshotProgress::Pending { written: 0 });
            }
            if record.kind == 0 {
                insert(
                    &mut self.tiles,
                    record.coordinate,
                    Tile::Solid(record.value),
                )?;
                self.build_at = record.next;
                self.build_count += 1;
                self.building = None;
            } else {
                let mut charge = self.scope.account();
                self.map_charge.resize(
                    self.map_charge
                        .bytes()
                        .checked_sub(DENSE_SAMPLES as u64)
                        .ok_or_else(limit)?,
                )?;
                charge.resize(DENSE_SAMPLES as u64)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(DENSE_SAMPLES)
                    .map_err(|_| limit())?;
                if values.capacity() != DENSE_SAMPLES {
                    drop(values);
                    return Err(limit());
                }
                values.resize(DENSE_SAMPLES, self.header.default);
                self.pending = Some(PendingDense {
                    record,
                    dense: Dense {
                        values,
                        written: record.samples(),
                        uniform: None,
                        mixed: true,
                        weighted: record.kind == 2,
                        _charge: charge,
                    },
                    at: 0,
                });
                self.building = None;
            }
        }
    }
    pub fn finish(mut self) -> Result<DecodedSnapshot> {
        if self.failed || !self.done || self.pending.is_some() {
            return Err(bad("selection decode incomplete"));
        }
        (self.callbacks.check)()?;
        let header = self.header;
        let selection = if let Some(domain) = header.clip {
            let storage = if self.tiles.is_empty() {
                None
            } else {
                let bytes = add(
                    map_layout(self.tiles.len())?.bytes,
                    (size_of::<Storage>() + 2 * size_of::<usize>()) as u64,
                )?;
                self.map_charge.resize(bytes)?;
                let total = add(bytes, mul(self.dense_count, DENSE_SAMPLES as u64)?)?;
                Some(Arc::new(Storage {
                    tiles: self.tiles,
                    bytes: total,
                    _charge: self.map_charge,
                }))
            };
            CapturedSelection::Field(if storage.is_none() {
                // Header retains the original view for service binding, while a
                // uniform field has no backing coordinates to subtract at sample time.
                BinaryMask::uniform(
                    domain,
                    (if header.inverted {
                        255 - header.default
                    } else {
                        header.default
                    }) == 255,
                )?
            } else {
                BinaryMask {
                    domain,
                    default: header.default,
                    inverted: header.inverted,
                    offset: header.offset,
                    storage,
                    base: None,
                }
            })
        } else {
            CapturedSelection::Empty
        };
        Ok(DecodedSnapshot { header, selection })
    }
}
fn insert(map: &mut HashMap<TileCoord, Tile>, coordinate: TileCoord, tile: Tile) -> Result<()> {
    // Exactly the old Builder's minimal insertion-only reserve provenance.
    if map.len() == map.capacity() {
        map.try_reserve(1).map_err(|_| limit())?;
    }
    if map.capacity() != map_layout(map.len() + 1)?.capacity {
        return Err(limit());
    }
    if map.insert(coordinate, tile).is_some() {
        return Err(bad("duplicate decoded tile"));
    }
    Ok(())
}
