//! Private selection-capture transport; neither PCND nor an editor mutation API.
use super::*;
use crate::raster::metadata_clone::fresh_map_layout;
use paintcraft_core::types::resources::BudgetScope;
use std::mem::size_of;
mod cursor;
mod decode;
pub use cursor::SnapshotCursor;
pub use decode::{CapturedSelection, DecodeQuote, DecodedSnapshot, SnapshotDecoder};

pub const HEADER_BYTES: usize = 128;
pub const RECORD_BYTES: usize = 32;
pub const DENSE_SAMPLES: usize = 256 * 256;
// Modeled source actions, not compiler instructions or timing bounds.
pub const HEADER_WORK: u64 = 1024;
pub const RECORD_WORK: u64 = 512;
pub const GRID_WORK: u64 = 256;
pub const SAMPLE_WORK: u64 = 32;
pub const BYTE_WORK: u64 = 8;
pub const DENSE_INIT_WORK: u64 = DENSE_SAMPLES as u64 + 128;

#[derive(Clone, Copy, Debug)]
pub struct SnapshotLimits {
    pub grid_cells: u64,
    pub records: u32,
    pub bytes: u64,
    pub work: u64,
    pub indivisible_work: u64,
}
impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            grid_cells: 262_144,
            records: 4096,
            bytes: 64 << 20,
            work: 67_108_864,
            indivisible_work: 131_072,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SnapshotQuote {
    pub bytes_upper: u64,
    pub control_bytes: u64,
    pub work_upper: u64,
    pub grid_cells: u64,
    pub indivisible_work: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotProgress {
    Pending { written: usize },
    Complete { written: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotHeader {
    pub layer_extent: Rect,
    pub clip: Option<Rect>,
    pub offset: (i64, i64),
    pub default: u8,
    pub inverted: bool,
    pub records: u32,
    pub total_bytes: u64,
}
impl SnapshotHeader {
    pub fn encode(&self) -> [u8; HEADER_BYTES] {
        let mut out = [0; HEADER_BYTES];
        out[..8].copy_from_slice(b"PCSCAP01");
        out[8..16].copy_from_slice(&self.total_bytes.to_be_bytes());
        out[16..20].copy_from_slice(&self.records.to_be_bytes());
        put_rect(&mut out[24..56], self.layer_extent);
        if let Some(clip) = self.clip {
            put_rect(&mut out[56..88], clip);
            out[106] = 1;
        }
        out[88..96].copy_from_slice(&self.offset.0.to_be_bytes());
        out[96..104].copy_from_slice(&self.offset.1.to_be_bytes());
        out[104] = self.default;
        out[105] = u8::from(self.inverted);
        out
    }
    fn parse(bytes: &[u8], limits: SnapshotLimits) -> Result<Self> {
        if bytes.len() < HEADER_BYTES
            || &bytes[..8] != b"PCSCAP01"
            || bytes[20..24]
                .iter()
                .chain(&bytes[107..128])
                .any(|b| *b != 0)
            || !matches!(bytes[104], 0 | 255)
            || bytes[105] > 1
            || bytes[106] > 1
        {
            return Err(bad("selection snapshot header"));
        }
        let layer_extent = get_rect(&bytes[24..56])?;
        let clip = if bytes[106] == 0 {
            if bytes[56..88].iter().any(|b| *b != 0) {
                return Err(bad("selection snapshot empty clip"));
            }
            None
        } else {
            let clip = get_rect(&bytes[56..88])?;
            if layer_extent.intersect(&clip) != Some(clip) {
                return Err(bad("selection snapshot clip outside layer"));
            }
            Some(clip)
        };
        let header = Self {
            layer_extent,
            clip,
            offset: (i64_at(bytes, 88), i64_at(bytes, 96)),
            default: bytes[104],
            inverted: bytes[105] != 0,
            records: u32_at(bytes, 16),
            total_bytes: u64_at(bytes, 8),
        };
        if header.total_bytes != bytes.len() as u64
            || header.total_bytes > limits.bytes
            || header.records > limits.records
            || header.clip.is_none() && header.records != 0
            || header.records == 0 && bytes.len() != HEADER_BYTES
        {
            return Err(bad("selection snapshot length/count"));
        }
        Ok(header)
    }
}
fn bad(message: &str) -> Error {
    schema(message)
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
fn i64_at(bytes: &[u8], at: usize) -> i64 {
    i64::from_be_bytes(bytes[at..at + 8].try_into().expect("checked fixed field"))
}
fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(bytes[at..at + 8].try_into().expect("checked fixed field"))
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().expect("checked fixed field"))
}
fn get_rect(bytes: &[u8]) -> Result<Rect> {
    let rect = Rect {
        x0: i64_at(bytes, 0),
        y0: i64_at(bytes, 8),
        x1: i64_at(bytes, 16),
        y1: i64_at(bytes, 24),
    };
    valid_domain(rect)?;
    Ok(rect)
}
fn put_rect(bytes: &mut [u8], rect: Rect) {
    for (part, value) in bytes
        .as_chunks_mut::<8>()
        .0
        .iter_mut()
        .zip([rect.x0, rect.y0, rect.x1, rect.y1])
    {
        part.copy_from_slice(&value.to_be_bytes());
    }
}
#[derive(Clone, Copy, Debug)]
struct Grid {
    x0: i64,
    x1: i64,
    y0: i64,
    y1: i64,
    cells: u64,
}
fn grid(clip: Rect, offset: (i64, i64)) -> Result<Grid> {
    let coordinate = |v: i64, delta: i64| {
        v.checked_sub(delta)
            .map(|n| n.div_euclid(256))
            .ok_or_else(limit)
    };
    let x0 = coordinate(clip.x0, offset.0)?;
    let y0 = coordinate(clip.y0, offset.1)?;
    let x1 = coordinate(clip.x1 - 1, offset.0)?;
    let y1 = coordinate(clip.y1 - 1, offset.1)?;
    let width = u64::try_from(
        x1.checked_sub(x0)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(limit)?,
    )
    .map_err(|_| limit())?;
    let height = u64::try_from(
        y1.checked_sub(y0)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(limit)?,
    )
    .map_err(|_| limit())?;
    Ok(Grid {
        x0,
        x1,
        y0,
        y1,
        cells: mul(width, height)?,
    })
}
fn tile_clip(tx: i64, ty: i64, offset: (i64, i64), clip: Rect) -> Result<Rect> {
    let origin = |t: i64, o: i64| {
        t.checked_mul(256)
            .and_then(|v| v.checked_add(o))
            .ok_or_else(limit)
    };
    let x0 = origin(tx, offset.0)?;
    let y0 = origin(ty, offset.1)?;
    let tile = Rect {
        x0,
        y0,
        x1: x0.checked_add(256).ok_or_else(limit)?,
        y1: y0.checked_add(256).ok_or_else(limit)?,
    };
    tile.intersect(&clip)
        .ok_or_else(|| bad("selection snapshot tile outside clip"))
}
fn map_layout(entries: usize) -> Result<crate::raster::metadata_clone::FreshMapLayout> {
    fresh_map_layout::<TileCoord, Tile>(entries)
}
fn lookup_headroom(entries: usize) -> Result<u64> {
    if entries == 0 {
        return Ok(GRID_WORK);
    }
    let capacity = map_layout(entries)?.capacity as u64;
    let buckets = if capacity < 8 {
        add(capacity, 1)?
    } else {
        mul(capacity / 7, 8)?
    };
    add(64, mul(buckets, 8)?)
}
#[derive(Clone, Copy)]
struct Callbacks<'a> {
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
}
impl std::fmt::Debug for Callbacks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Callbacks").finish_non_exhaustive()
    }
}
struct Meter<'a> {
    quantum: u64,
    work: &'a mut u64,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
}
impl Meter<'_> {
    fn take(&mut self, amount: u64, headroom: u64) -> Result<bool> {
        if self.quantum < amount.max(headroom) {
            return Ok(false);
        }
        (self.check)()?;
        if *self.work < amount {
            return Err(limit());
        }
        (self.spend)(amount)?;
        *self.work -= amount;
        self.quantum -= amount;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
