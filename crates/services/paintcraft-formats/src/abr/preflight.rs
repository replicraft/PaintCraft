use super::*;

pub(super) struct Cursor<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}
impl<'a> Cursor<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| invalid("ABR extent overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated ABR payload"))?;
        self.pos = end;
        Ok(bytes)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn zeros(&mut self, n: usize) -> Result<()> {
        if self.take(n)?.iter().any(|b| *b != 0) {
            return Err(invalid("nonzero ABR padding"));
        }
        Ok(())
    }
    pub fn finished(&self) -> Result<()> {
        if self.pos != self.bytes.len() {
            return Err(invalid("ABR payload has trailing data"));
        }
        Ok(())
    }
}
pub(super) struct Prepared {
    pub version: u16,
    pub minor_version: u16,
    pub samples: Vec<Sample>,
    pub sections: Vec<Section>,
    pub normalize: bool,
}
pub(super) fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Prepared> {
    check()?;
    if bytes.len() > BYTES_MAX {
        return Err(resource("ABR source allowance exceeded"));
    }
    let mut source = Cursor { bytes, pos: 0 };
    let version = source.u16()?;
    if !matches!(version, 6 | 7 | 9 | 10) {
        return Err(unsupported("unsupported ABR version"));
    }
    let minor_version = source.u16()?;
    if !matches!(minor_version, 1 | 2) {
        return Err(unsupported("unsupported ABR minor version"));
    }
    let mut prepared = Prepared {
        version,
        minor_version,
        samples: Vec::new(),
        sections: Vec::new(),
        normalize: false,
    };
    let mut pixels = 0;
    let mut descriptor_budget = descriptor::Budget::default();
    while source.pos < bytes.len() {
        check()?;
        if prepared.sections.len() == SECTIONS_MAX {
            return Err(resource("ABR section allowance exceeded"));
        }
        if source.take(4)? != b"8BIM" {
            return Err(invalid("invalid ABR section signature"));
        }
        let kind = source.take(4)?;
        let size = source.u32()? as usize;
        let payload = source.take(size)?;
        let mut section = Cursor {
            bytes: payload,
            pos: 0,
        };
        let kind = match kind {
            b"samp" => {
                while section.pos < payload.len() {
                    check()?;
                    if prepared.samples.len() == SAMPLES_MAX {
                        return Err(resource("ABR sample allowance exceeded"));
                    }
                    let size = section.u32()? as usize;
                    let data = section.take(size)?;
                    let (sample, normalize) = sample(data, minor_version, &mut pixels, check)?;
                    prepared.normalize |= normalize;
                    section.zeros((4 - size % 4) % 4)?;
                    prepared.samples.push(sample);
                }
                SectionKind::Samples
            }
            b"desc" | b"phry" => {
                descriptor::inspect(&mut section, &mut descriptor_budget, check)?;
                if kind == b"desc" {
                    SectionKind::Descriptors
                } else {
                    SectionKind::Hierarchy
                }
            }
            b"patt" => {
                return Err(unsupported(
                    "ABR embedded patterns require bounded pattern qualification",
                ));
            }
            _ => return Err(unsupported("unknown ABR section kind")),
        };
        section.finished()?;
        source.zeros((4 - size % 4) % 4)?;
        prepared.sections.push(Section {
            kind,
            payload: payload.to_vec(),
        });
    }
    Ok(prepared)
}
pub(super) fn sample(
    bytes: &[u8],
    minor: u16,
    total: &mut usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Sample, bool)> {
    let mut source = Cursor { bytes, pos: 0 };
    let n = usize::from(source.take(1)?[0]);
    let id = source.take(n)?;
    if !id.iter().all(|b| b.is_ascii() && *b != 0) {
        return Err(invalid("ABR sample ID must be null-free ASCII"));
    }
    let id = String::from_utf8(id.to_vec()).unwrap();
    source.take(if minor == 1 { 10 } else { 264 })?;
    let bounds = [
        source.u32()? as i32,
        source.u32()? as i32,
        source.u32()? as i32,
        source.u32()? as i32,
    ];
    let width = i64::from(bounds[3]) - i64::from(bounds[1]);
    let height = i64::from(bounds[2]) - i64::from(bounds[0]);
    if width <= 0 || height <= 0 {
        return Err(invalid("invalid ABR sample bounds"));
    }
    if width > DIMENSION_MAX as i64 || height > DIMENSION_MAX as i64 {
        return Err(resource("ABR sample dimension allowance exceeded"));
    }
    let (width, height) = (width as usize, height as usize);
    let pixels = width * height;
    *total += pixels;
    if pixels > SAMPLE_PIXELS_MAX || *total > TOTAL_PIXELS_MAX {
        return Err(resource("ABR sample pixel allowance exceeded"));
    }
    let depth = source.u16()?;
    let compression = source.take(1)?[0];
    let alpha = match (depth, compression) {
        (8, 0) => Alpha::U8(source.take(pixels)?.to_vec()),
        (16, 0) => {
            let mut values = Vec::new();
            for chunk in source.take(pixels * 2)?.chunks(width * 2) {
                check()?;
                values.extend(
                    chunk
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|v| u16::from_be_bytes(*v)),
                );
            }
            Alpha::U16(values)
        }
        (8, 1) => Alpha::U8(rle(&mut source, width, height, check)?),
        (16, 1) => {
            let bytes = rle(&mut source, width * 2, height, check)?;
            let mut values = Vec::new();
            for row in bytes.chunks(width * 2) {
                check()?;
                values.extend(
                    row.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|v| u16::from_be_bytes(*v)),
                );
            }
            Alpha::U16(values)
        }
        _ => return Err(unsupported("unsupported ABR sample depth/compression")),
    };
    if bytes.len() - source.pos > 3 {
        return Err(invalid("ABR sample has unexplained trailing data"));
    }
    source.zeros(bytes.len() - source.pos)?;
    Ok((
        Sample { id, bounds, alpha },
        depth == 16 && compression == 1,
    ))
}
fn rle(
    source: &mut Cursor<'_>,
    width: usize,
    height: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let lengths = source.take(height * 2)?;
    let mut alpha = Vec::new();
    for length in lengths.as_chunks::<2>().0 {
        check()?;
        let mut row = Cursor {
            bytes: source.take(usize::from(u16::from_be_bytes(*length)))?,
            pos: 0,
        };
        let start = alpha.len();
        while row.pos < row.bytes.len() {
            let header = row.take(1)?[0];
            let count = match header {
                0..=127 => usize::from(header) + 1,
                128 => 0,
                _ => 257 - usize::from(header),
            };
            if alpha.len() - start + count > width {
                return Err(invalid("ABR PackBits packet crosses a row"));
            }
            match header {
                0..=127 => alpha.extend(row.take(count)?),
                128 => {}
                _ => {
                    let value = row.take(1)?[0];
                    alpha.resize(alpha.len() + count, value);
                }
            }
        }
        if alpha.len() - start != width {
            return Err(invalid("ABR PackBits row is incomplete"));
        }
    }
    Ok(alpha)
}
