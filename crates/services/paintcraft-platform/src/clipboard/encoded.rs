//! Frozen encoded transport. No codec parsing or pixel decoding occurs here.
use paintcraft_core::types::resources::{BudgetLease, SharedBudget};
use std::{fmt, sync::Arc};

pub const MAX_ENCODED_BYTES: u64 = 64 << 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Png,
    Tiff,
    Dib,
    DibV5,
}
impl Encoding {
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Tiff => "image/tiff",
            Self::Dib | Self::DibV5 => "image/x-dib",
        }
    }
    fn matches(self, bytes: &[u8]) -> bool {
        match self {
            Self::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            Self::Tiff => bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*"),
            Self::Dib | Self::DibV5 => bytes
                .get(..4)
                .and_then(|v| v.try_into().ok())
                .map(u32::from_le_bytes)
                .is_some_and(|size| {
                    if self == Self::DibV5 {
                        size == 124
                    } else {
                        matches!(size, 40 | 108 | 124)
                    }
                }),
        }
    }
}
struct Payload {
    bytes: Vec<u8>,
    _lease: BudgetLease,
    _extra: Option<BudgetLease>,
}
#[derive(Clone)]
pub struct EncodedImage {
    encoding: Encoding,
    payload: Arc<Payload>,
}
impl fmt::Debug for EncodedImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncodedImage")
            .field("encoding", &self.encoding)
            .field("byte_length", &self.payload.bytes.len())
            .finish_non_exhaustive()
    }
}
impl EncodedImage {
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }
    pub fn bytes(&self) -> &[u8] {
        &self.payload.bytes
    }
    /// Admit advertised length before copying. The adapter must retain immutable
    /// source bytes through the copy and separately reject ownership changes.
    pub fn capture(
        encoding: Encoding,
        length: usize,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
        read: impl FnOnce() -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        check()?;
        if length == 0 || length as u64 > MAX_ENCODED_BYTES {
            return Err("Clipboard encoded image exceeds the limit".into());
        }
        let lease = budget.reserve(length as u64).map_err(|e| e.to_string())?;
        check()?;
        let bytes = read()?;
        if bytes.len() != length {
            return Err("Clipboard image changed during read".into());
        }
        if bytes.capacity() as u64 > MAX_ENCODED_BYTES {
            return Err("Clipboard allocation exceeds the limit".into());
        }
        let extra = if bytes.capacity() > length {
            Some(
                budget
                    .reserve((bytes.capacity() - length) as u64)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        check()?;
        if !encoding.matches(&bytes) {
            return Err("Clipboard image signature does not match its advertised type".into());
        }
        Ok(Self {
            encoding,
            payload: Arc::new(Payload {
                bytes,
                _lease: lease,
                _extra: extra,
            }),
        })
    }
}
#[cfg(test)]
mod tests;

mod stream;
