//! Decoder intake selects the initial IFD and, for raw gray-alpha mapping,
//! temporarily interprets its photometric tag as BlackIsZero. Subsequent
//! payload and aliased metadata reads address original immutable source bytes.
use super::*;
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use std::{cell::Cell, rc::Rc};

struct Photometric {
    at: usize,
    size: usize,
    value: [u8; 4],
    active: Rc<Cell<bool>>,
}
pub(super) struct Selected<'a> {
    source: &'a [u8],
    header: [u8; 8],
    position: usize,
    initial_header: bool,
    photometric: Option<Photometric>,
    check: &'a dyn Fn() -> Result<()>,
}
impl<'a> Selected<'a> {
    pub fn new(
        source: &'a [u8],
        descriptor: Descriptor,
        check: &'a dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let mut header: [u8; 8] = source
            .get(..8)
            .ok_or_else(|| malformed("TIFF header is truncated"))?
            .try_into()
            .unwrap();
        let offset = u32::try_from(descriptor.ifd_offset)
            .map_err(|_| unsupported("BigTIFF requires a separately qualified profile"))?;
        if offset < 8 || offset as usize >= source.len() {
            return Err(malformed("TIFF selected IFD is outside its source"));
        }
        header[4..8].copy_from_slice(&if descriptor.little_endian {
            offset.to_le_bytes()
        } else {
            offset.to_be_bytes()
        });
        Ok(Self {
            source,
            header,
            position: 0,
            initial_header: true,
            photometric: None,
            check,
        })
    }
    /// TIFF0.11 reports gray+alpha as Multiband and refuses WhiteIsZero
    /// inversion for it. Interpret the selected metadata as BlackIsZero only
    /// during decoder construction; decode raw bytes and invert gray ourselves.
    pub fn gray_alpha(
        source: &'a [u8],
        descriptor: Descriptor,
        check: &'a dyn Fn() -> Result<()>,
    ) -> Result<(Self, Rc<Cell<bool>>)> {
        let mut reader = Self::new(source, descriptor, check)?;
        let active = Rc::new(Cell::new(true));
        if descriptor.photometric == 0 {
            let word = |at| {
                let bytes: [u8; 2] = source[at..at + 2].try_into().unwrap();
                if descriptor.little_endian {
                    u16::from_le_bytes(bytes)
                } else {
                    u16::from_be_bytes(bytes)
                }
            };
            let ifd = descriptor.ifd_offset as usize;
            for index in 0..word(ifd) as usize {
                let at = ifd + 2 + index * 12;
                if word(at) == 262 {
                    let size = if word(at + 2) == 3 { 2 } else { 4 };
                    let mut value = [0; 4];
                    if descriptor.little_endian {
                        value[0] = 1;
                    } else {
                        value[size - 1] = 1;
                    }
                    reader.photometric = Some(Photometric {
                        at: at + 8,
                        size,
                        value,
                        active: active.clone(),
                    });
                    break;
                }
            }
            if reader.photometric.is_none() {
                return Err(malformed(
                    "TIFF grayscale photometric declaration disappeared",
                ));
            }
        }
        Ok((reader, active))
    }
    fn check(&self) -> io::Result<()> {
        (self.check)().map_err(|error| io::Error::other(error.to_string()))
    }
}
impl Read for Selected<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let bytes = self.fill_buf()?;
        let count = output.len().min(bytes.len());
        output[..count].copy_from_slice(&bytes[..count]);
        self.consume(count);
        Ok(count)
    }
}
impl BufRead for Selected<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.check()?;
        if self.initial_header && self.position < 8 {
            return Ok(&self.header[self.position..]);
        }
        if let Some(projection) = &self.photometric
            && projection.active.get()
        {
            if self.position < projection.at {
                return Ok(&self.source[self.position..projection.at]);
            }
            if self.position < projection.at + projection.size {
                return Ok(&projection.value[self.position - projection.at..projection.size]);
            }
        }
        Ok(self.source.get(self.position..).unwrap_or_default())
    }
    fn consume(&mut self, amount: usize) {
        self.position = self.position.saturating_add(amount).min(self.source.len());
        if self.position >= 8 {
            self.initial_header = false;
        }
    }
}
impl Seek for Selected<'_> {
    fn seek(&mut self, seek: SeekFrom) -> io::Result<u64> {
        self.check()?;
        let position = match seek {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::End(value) => self.source.len() as i128 + i128::from(value),
            SeekFrom::Current(value) => self.position as i128 + i128::from(value),
        };
        if position < 0 || position > self.source.len() as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "TIFF seek is outside the captured source",
            ));
        }
        self.position = position as usize;
        Ok(self.position as u64)
    }
}
