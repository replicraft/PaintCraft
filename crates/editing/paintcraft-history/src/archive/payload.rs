use super::*;
use paintcraft_raster::{Admission, TileLease};
use std::sync::Mutex;
const EDGE: usize = paintcraft_raster::TILE_EDGE as usize;
const ROW_BYTES: usize = EDGE * 4;

#[derive(Debug)]
enum Payload {
    Shared(TileLease),
    Solid(Rgba),
    Patch {
        default: Rgba,
        rect: (u16, u16, u16, u16),
        bytes: Box<[u8]>,
    },
}
#[derive(Debug)]
pub(crate) struct Blob {
    payload: Payload,
    crc: u32,
    decoded: Mutex<TileVersion>,
}
impl Blob {
    pub fn pack(
        lease: TileLease,
        default: Rgba,
        budget: &mut usize,
        check: &impl Fn() -> Result<()>,
    ) -> Result<Self> {
        let data = lease.bytes();
        let crc = crc32fast::hash(data);
        let first = Rgba(data[..4].try_into().expect("full tile"));
        let payload = if data.as_chunks::<4>().0.iter().all(|p| *p == first.0) {
            Payload::Solid(first)
        } else {
            let (mut x0, mut y0, mut x1, mut y1) = (EDGE, EDGE, 0usize, 0usize);
            for (y, row) in data.as_chunks::<ROW_BYTES>().0.iter().enumerate() {
                check()?;
                for (x, px) in row.as_chunks::<4>().0.iter().enumerate() {
                    if *px != default.0 {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x + 1);
                        y1 = y1.max(y + 1);
                    }
                }
            }
            let bytes = (x1 - x0) * (y1 - y0) * 4;
            // Whole high-entropy tiles retain their existing immutable allocation.
            if bytes + 32 >= RasterLayer::TILE_BYTES {
                Payload::Shared(lease.clone())
            } else {
                if bytes > *budget {
                    return Err(limit("history patch admission exceeded"));
                }
                let mut out = Vec::new();
                out.try_reserve_exact(bytes)
                    .map_err(|_| limit("cannot allocate history patch"))?;
                if out.capacity() > *budget {
                    return Err(limit("history patch capacity exceeded"));
                }
                *budget -= out.capacity();
                for y in y0..y1 {
                    out.extend_from_slice(&data[(y * EDGE + x0) * 4..(y * EDGE + x1) * 4]);
                }
                Payload::Patch {
                    default,
                    rect: (x0 as u16, y0 as u16, (x1 - x0) as u16, (y1 - y0) as u16),
                    bytes: out.into_boxed_slice(),
                }
            }
        };
        Ok(Self {
            payload,
            crc,
            decoded: Mutex::new(lease.version()),
        })
    }
    pub fn owns_decoded(&self) -> bool {
        matches!(self.payload, Payload::Shared(_))
    }
    pub fn encoded_bytes(&self) -> usize {
        match &self.payload {
            Payload::Shared(_) => 0,
            Payload::Solid(_) => 0,
            Payload::Patch { bytes, .. } => bytes.len(),
        }
    }
    pub fn source(&self) -> TileVersion {
        self.decoded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn restore(&self, adm: &mut Admission) -> Result<TileLease> {
        if let Some(lease) = self.source().try_lease() {
            return Ok(lease);
        }
        let (default, patch) = match &self.payload {
            Payload::Shared(lease) => return Ok(lease.clone()),
            Payload::Solid(color) => (*color, None),
            Payload::Patch {
                default,
                rect,
                bytes,
            } => (*default, Some((rect, bytes))),
        };
        let tile = TileLease::materialize(default, adm, |out| {
            if let Some((&(x, y, w, h), bytes)) = patch {
                let (x, y, w, h) = (
                    usize::from(x),
                    usize::from(y),
                    usize::from(w),
                    usize::from(h),
                );
                if w == 0 || h == 0 || x + w > EDGE || y + h > EDGE || bytes.len() != w * h * 4 {
                    return Err(corrupt());
                }
                for row in 0..h {
                    out[((y + row) * EDGE + x) * 4..((y + row) * EDGE + x + w) * 4]
                        .copy_from_slice(&bytes[row * w * 4..(row + 1) * w * 4]);
                }
            }
            if crc32fast::hash(out) != self.crc {
                return Err(corrupt());
            }
            Ok(())
        })?;
        *self.decoded.lock().unwrap_or_else(|p| p.into_inner()) = tile.version();
        Ok(tile)
    }
}
fn corrupt() -> Error {
    Error::new(
        ErrorCode::CodecFailed,
        "recorded history tile failed integrity validation",
    )
}

#[cfg(test)]
pub(super) fn corrupt_patch_for_test(blob: &mut Blob) {
    let Payload::Patch { bytes, .. } = &mut blob.payload else {
        panic!("expected patch");
    };
    bytes[0] ^= 1;
}
