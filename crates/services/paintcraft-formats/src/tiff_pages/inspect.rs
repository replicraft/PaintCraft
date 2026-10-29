//! Complete bounded classic-IFD graph inspection, without decoder construction.
use super::*;

#[derive(Clone, Copy)]
struct Field {
    kind: u16,
    count: u32,
    data: usize,
    length: usize,
}
struct Reader<'a> {
    bytes: &'a [u8],
    little: bool,
}
impl Reader<'_> {
    fn bytes(&self, at: usize, length: usize) -> Result<&[u8]> {
        let end = at
            .checked_add(length)
            .ok_or_else(|| malformed("TIFF offset overflow"))?;
        self.bytes
            .get(at..end)
            .ok_or_else(|| malformed("TIFF field is outside the source"))
    }
    fn word(&self, at: usize) -> Result<u16> {
        let bytes = self.bytes(at, 2)?.try_into().unwrap();
        Ok(if self.little {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    }
    fn long(&self, at: usize) -> Result<u32> {
        let bytes = self.bytes(at, 4)?.try_into().unwrap();
        Ok(if self.little {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }
    fn integer(&self, field: Field, index: usize) -> Result<u64> {
        if index >= field.count as usize {
            return Err(malformed("TIFF integer index overflow"));
        }
        Ok(match field.kind {
            3 => u64::from(self.word(field.data + index * 2)?),
            4 | 13 => u64::from(self.long(field.data + index * 4)?),
            16 | 18 => {
                let bytes = self.bytes(field.data + index * 8, 8)?.try_into().unwrap();
                if self.little {
                    u64::from_le_bytes(bytes)
                } else {
                    u64::from_be_bytes(bytes)
                }
            }
            _ => return Err(unsupported("TIFF integer field type is not qualified")),
        })
    }
    fn scalar(&self, field: Field) -> Option<u32> {
        (field.count == 1 && matches!(field.kind, 3 | 4))
            .then(|| {
                self.integer(field, 0)
                    .ok()
                    .and_then(|v| u32::try_from(v).ok())
            })
            .flatten()
    }
    fn components(&self, field: Field) -> Components {
        let mut result = Components {
            count: field.count,
            values: [0; 4],
        };
        for (index, value) in result
            .values
            .iter_mut()
            .enumerate()
            .take((field.count as usize).min(4))
        {
            *value = self
                .integer(field, index)
                .ok()
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0);
        }
        result
    }
    fn field(&self, at: usize) -> Result<Field> {
        let kind = self.word(at + 2)?;
        let count = self.long(at + 4)?;
        let size = match kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 | 16 | 17 | 18 => 8,
            _ => {
                return Err(unsupported(
                    "TIFF field type has no qualified structural size",
                ));
            }
        };
        let length = (count as usize)
            .checked_mul(size)
            .ok_or_else(|| resource("TIFF tag byte count overflow"))?;
        if length > METADATA_PER_PAGE_MAX {
            return Err(resource("TIFF tag exceeds metadata allowance"));
        }
        let data = if length <= 4 {
            at + 8
        } else {
            self.long(at + 8)? as usize
        };
        self.bytes(data, length)?;
        Ok(Field {
            kind,
            count,
            data,
            length,
        })
    }
    fn chunks(
        &self,
        offsets: Option<Field>,
        lengths: Option<Field>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        let (offsets, lengths) = match (offsets, lengths) {
            (None, None) => return Ok(false),
            (Some(a), Some(b)) if a.count == b.count => (a, b),
            _ => return Err(malformed("TIFF chunk offset/count arrays differ")),
        };
        let mut sparse = false;
        for index in 0..offsets.count as usize {
            check()?;
            let start = self.integer(offsets, index)?;
            let length = self.integer(lengths, index)?;
            if start == 0 && length == 0 {
                sparse = true;
                continue;
            }
            if start < 8
                || length == 0
                || start
                    .checked_add(length)
                    .is_none_or(|end| end > self.bytes.len() as u64)
            {
                return Err(malformed("TIFF encoded chunk is outside its full source"));
            }
        }
        Ok(sparse)
    }
}

pub fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Inventory> {
    check()?;
    if bytes.len() < 8 || bytes.len() > BYTES_MAX {
        return Err(resource("TIFF source exceeds selected-page byte allowance"));
    }
    let little = if bytes.starts_with(b"II*\0") {
        true
    } else if bytes.starts_with(b"MM\0*") {
        false
    } else if bytes.starts_with(b"II+\0") || bytes.starts_with(b"MM\0+") {
        return Err(unsupported(
            "BigTIFF requires a separately qualified profile",
        ));
    } else {
        return Err(malformed("TIFF classic header is invalid"));
    };
    let reader = Reader { bytes, little };
    let mut offset = reader.long(4)? as usize;
    let mut pages = Vec::new();
    pages
        .try_reserve_exact(PAGES_MAX)
        .map_err(|_| resource("TIFF page inventory allocation"))?;
    let mut ranges = [(0usize, 0usize); PAGES_MAX];
    let mut total_tags = 0usize;
    let mut total_metadata = 0usize;
    while offset != 0 {
        check()?;
        if pages.len() == PAGES_MAX {
            return Err(resource("TIFF page count exceeds its allowance"));
        }
        if offset < 8 {
            return Err(malformed("TIFF IFD overlaps its header"));
        }
        let count = reader.word(offset)? as usize;
        if count > TAGS_PER_PAGE_MAX {
            return Err(resource("TIFF page has too many tags"));
        }
        total_tags = total_tags
            .checked_add(count)
            .ok_or_else(|| resource("TIFF tag count overflow"))?;
        if total_tags > TAGS_TOTAL_MAX {
            return Err(resource("TIFF total tag count exceeds its allowance"));
        }
        let end = offset
            .checked_add(2 + count * 12 + 4)
            .ok_or_else(|| malformed("TIFF IFD extent overflow"))?;
        reader.bytes(offset, end - offset)?;
        if ranges[..pages.len()]
            .iter()
            .any(|&(start, stop)| offset < stop && start < end)
        {
            return Err(malformed(
                "TIFF IFD graph cycles or overlapping directories",
            ));
        }
        ranges[pages.len()] = (offset, end);
        let page = page(
            &reader,
            offset,
            count,
            pages.len() as u16,
            &mut total_metadata,
            check,
        )?;
        pages.push(page);
        offset = reader.long(end - 4)? as usize;
    }
    if pages.is_empty() {
        return Err(malformed("TIFF contains no image directories"));
    }
    let source_sha256 = hash_checked(bytes, check)?;
    Ok(Inventory {
        pages,
        source_sha256,
        source_bytes: bytes.len() as u64,
    })
}

fn page(
    reader: &Reader<'_>,
    offset: usize,
    count: usize,
    index: u16,
    total_metadata: &mut usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Page> {
    let mut descriptor = Descriptor {
        ifd_offset: offset as u64,
        little_endian: reader.little,
        storage: 0,
        bits_per_sample: Components::single(1),
        sample_format: Components::single(1),
        extra_samples: Components {
            count: 0,
            values: [0; 4],
        },
        photometric: u32::MAX,
        samples_per_pixel: 1,
        compression: 1,
        orientation: 1,
        planar_configuration: 1,
        icc_bytes: 0,
        icc_sha256: [0; 32],
        metadata_bytes: 0,
        omitted_metadata: false,
        resolution_x: None,
        resolution_y: None,
        resolution_unit: None,
    };
    let (mut width, mut height) = (None, None);
    let (mut strips, mut strip_bytes, mut tiles, mut tile_bytes) = (None, None, None, None);
    let mut seen = [0u64; 1024];
    let mut metadata = 0usize;
    let (mut rows_per_strip, mut tile_width, mut tile_height, mut predictor) =
        (u32::MAX, None, None, 1);
    for entry in 0..count {
        check()?;
        let at = offset + 2 + entry * 12;
        let tag = reader.word(at)?;
        let slot = usize::from(tag / 64);
        let bit = 1u64 << (tag % 64);
        if seen[slot] & bit != 0 {
            return Err(malformed("duplicate TIFF tag in a page"));
        }
        seen[slot] |= bit;
        let field = reader.field(at)?;
        metadata = metadata
            .checked_add(field.length)
            .ok_or_else(|| resource("TIFF page metadata overflow"))?;
        *total_metadata = total_metadata
            .checked_add(field.length)
            .ok_or_else(|| resource("TIFF aggregate metadata overflow"))?;
        if metadata > METADATA_PER_PAGE_MAX || *total_metadata > METADATA_BYTES_MAX {
            return Err(resource("TIFF metadata exceeds its declared allowance"));
        }
        match tag {
            256 => width = reader.scalar(field),
            257 => height = reader.scalar(field),
            258 => descriptor.bits_per_sample = reader.components(field),
            259 => descriptor.compression = reader.scalar(field).unwrap_or(u32::MAX),
            262 => descriptor.photometric = reader.scalar(field).unwrap_or(u32::MAX),
            274 => descriptor.orientation = reader.scalar(field).unwrap_or(u32::MAX),
            277 => descriptor.samples_per_pixel = reader.scalar(field).unwrap_or(u32::MAX),
            284 => descriptor.planar_configuration = reader.scalar(field).unwrap_or(u32::MAX),
            338 => descriptor.extra_samples = reader.components(field),
            339 => descriptor.sample_format = reader.components(field),
            278 => rows_per_strip = reader.scalar(field).unwrap_or(0),
            317 => predictor = reader.scalar(field).unwrap_or(0),
            322 => tile_width = reader.scalar(field),
            323 => tile_height = reader.scalar(field),
            273 => strips = Some(field),
            279 => strip_bytes = Some(field),
            324 => tiles = Some(field),
            325 => tile_bytes = Some(field),
            34675 => {
                descriptor.icc_bytes = field.length as u32;
                descriptor.icc_sha256 =
                    hash_checked(reader.bytes(field.data, field.length)?, check)?;
            }
            _ => {}
        }
        if matches!(tag, 282 | 283) && field.kind == 5 && field.count == 1 {
            let resolution = (reader.long(field.data)?, reader.long(field.data + 4)?);
            if tag == 282 {
                descriptor.resolution_x = Some(resolution);
            } else {
                descriptor.resolution_y = Some(resolution);
            }
        }
        if tag == 296 {
            descriptor.resolution_unit = reader.scalar(field);
        }
        if !matches!(tag, 256..=259 | 262 | 266 | 273 | 274 | 277..=279 | 284 | 317 | 322..=325 | 330 | 338 | 339 | 34665 | 34675 | 34853 | 40965)
        {
            descriptor.omitted_metadata = true;
        }
    }
    let sparse =
        reader.chunks(strips, strip_bytes, check)? | reader.chunks(tiles, tile_bytes, check)?;
    descriptor.storage = u8::from(strips.is_some()) | (u8::from(tiles.is_some()) << 1);
    let width = width
        .filter(|v| *v != 0)
        .ok_or_else(|| malformed("TIFF page has no valid width"))?;
    let height = height
        .filter(|v| *v != 0)
        .ok_or_else(|| malformed("TIFF page has no valid height"))?;
    descriptor.metadata_bytes = metadata as u32;
    let geometry = match (strips, tiles) {
        (Some(offsets), None) if rows_per_strip != 0 => {
            offsets.count == height.div_ceil(rows_per_strip)
        }
        (None, Some(offsets)) => match (tile_width, tile_height) {
            (Some(w), Some(h))
                if w != 0
                    && h != 0
                    && w.is_multiple_of(16)
                    && h.is_multiple_of(16)
                    && u64::from(w) * u64::from(h) <= PIXELS_MAX =>
            {
                u64::from(offsets.count)
                    == u64::from(width.div_ceil(w)) * u64::from(height.div_ceil(h))
            }
            _ => false,
        },
        _ => false,
    };
    let result = crate::image::tiff::admit_page(reader.bytes, offset, PIXELS_MAX, &|| Ok(()));
    check()?;
    let unsupported_reason = match result {
        Ok(_)
            if assessment::qualified(&descriptor)
                && geometry
                && !sparse
                && matches!(predictor, 1 | 2)
                && (predictor != 2 || descriptor.bits_per_sample.values[0] != 1) =>
        {
            None
        }
        Ok(_) => Some("TIFF component/storage/compression profile is not qualified"),
        Err(_) => Some("TIFF page is outside the qualified integer/straight-alpha profile"),
    };
    Ok(Page {
        index,
        width,
        height,
        descriptor,
        unsupported_reason,
    })
}
