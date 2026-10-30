//! Minimal, bounded wire primitives. Integers use canonical unsigned LEB128.
use super::*;

pub(super) struct Output<W> {
    sink: W,
    hash: Sha256,
    metadata: usize,
    metadata_only: bool,
}
impl<W: Write> Output<W> {
    pub fn new(sink: W) -> Self {
        Self {
            sink,
            hash: Sha256::new(),
            metadata: 0,
            metadata_only: false,
        }
    }
    pub fn metadata_counter(sink: W) -> Self {
        Self {
            metadata_only: true,
            ..Self::new(sink)
        }
    }
    pub fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.metadata = self
            .metadata
            .checked_add(bytes.len())
            .filter(|size| *size <= DOCUMENT_MAX as usize)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "compact metadata exceeds its size limit",
                )
            })?;
        self.payload(bytes)
    }
    pub fn payload(&mut self, bytes: &[u8]) -> Result<()> {
        if self.metadata_only {
            return Ok(());
        }
        self.sink
            .write_all(bytes)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "cannot write native project"))?;
        self.hash.update(bytes);
        Ok(())
    }
    pub fn number(&mut self, mut value: u64) -> Result<()> {
        let mut bytes = [0u8; 10];
        let mut at = 0;
        while value >= 128 {
            bytes[at] = (value as u8 & 127) | 128;
            value >>= 7;
            at += 1;
        }
        bytes[at] = value as u8;
        self.bytes(&bytes[..=at])
    }
    pub fn signed(&mut self, value: i64) -> Result<()> {
        if value.unsigned_abs() > COORD_ABS_MAX as u64 {
            return Err(codec("origin exceeds coordinate limits"));
        }
        self.number(((value << 1) ^ (value >> 63)) as u64)
    }
    pub fn string(&mut self, value: &str) -> Result<()> {
        if value.len() > STRING_BYTES_MAX {
            return Err(codec("compact string exceeds its byte limit"));
        }
        self.number(value.len() as u64)?;
        self.bytes(value.as_bytes())
    }
    pub fn finish(mut self) -> Result<()> {
        if self.metadata_only {
            return Ok(());
        }
        self.sink
            .write_all(&self.hash.finalize())
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "cannot finish native project"))
    }
}

#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    bytes: &'a [u8],
}
impl<'a> Input<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    pub fn rest(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn remaining(&self) -> usize {
        self.bytes.len()
    }
    pub fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let result = self
            .bytes
            .get(..length)
            .ok_or_else(|| codec("truncated compact PCND"))?;
        self.bytes = &self.bytes[length..];
        Ok(result)
    }
    pub fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn number(&mut self, maximum: u64) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..70).step_by(7) {
            let byte = self.byte()?;
            if shift == 63 && byte > 1 {
                return Err(codec("compact integer overflows"));
            }
            value |= u64::from(byte & 127) << shift;
            if byte & 128 == 0 {
                if (shift != 0 && byte == 0) || value > maximum {
                    return Err(codec(
                        "compact integer exceeds its range or is noncanonical",
                    ));
                }
                return Ok(value);
            }
        }
        Err(codec("invalid compact integer"))
    }
    pub fn signed(&mut self) -> Result<i64> {
        let value = self.number(COORD_ABS_MAX as u64 * 2)?;
        Ok(((value >> 1) as i64) ^ -((value & 1) as i64))
    }
    pub fn dimension(&mut self) -> Result<u32> {
        let value = self.number(1_048_576)?;
        if value == 0 {
            return Err(codec("zero compact dimension"));
        }
        Ok(value as u32)
    }
    pub fn id(&mut self) -> Result<Uuid> {
        Ok(Uuid::from_bytes(self.take(16)?.try_into().unwrap()))
    }
    pub fn string(&mut self, chars: usize) -> Result<String> {
        let length = self.number(STRING_BYTES_MAX as u64)? as usize;
        let string =
            std::str::from_utf8(self.take(length)?).map_err(|_| codec("invalid compact UTF-8"))?;
        if string.chars().count() > chars {
            return Err(codec("compact string exceeds its character limit"));
        }
        Ok(string.to_owned())
    }
}
