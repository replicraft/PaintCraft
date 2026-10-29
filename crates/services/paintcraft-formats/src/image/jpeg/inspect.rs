use super::*;

struct Inspector<'a> {
    bytes: &'a [u8],
    limits: Limits,
    check: &'a dyn Fn() -> Result<()>,
    projection: Option<&'a mut [u8]>,
    frame: Option<Frame>,
    width: u32,
    height: u32,
    components: [Component; 4],
    count: u8,
    adobe: Option<u8>,
    density: Option<Density>,
    exif: Option<u8>,
    metadata: Metadata,
    icc: [Option<(usize, usize)>; 255],
    icc_count: Option<u8>,
    scans: u64,
}
pub fn inspect(bytes: &[u8], pixels: u64, check: &dyn Fn() -> Result<()>) -> Result<Source> {
    inspect_with_limits(bytes, Limits::for_pixels(pixels), check)
}
pub fn inspect_with_limits(
    bytes: &[u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Source> {
    check()?;
    if bytes.len() as u64 > limits.input_bytes.min(INPUT_MAX) {
        return Err(resource("JPEG encoded input exceeds its byte allowance"));
    }
    if !bytes.starts_with(b"\xff\xd8") {
        return Err(malformed("JPEG SOI header is missing"));
    }
    Inspector::new(bytes, limits, check, None).run()
}
pub(super) fn projected_copy(
    bytes: &[u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Source, Vec<u8>)> {
    check()?;
    if bytes.len() as u64 > limits.input_bytes.min(INPUT_MAX) || bytes.len() > isize::MAX as usize {
        return Err(resource(
            "JPEG decoder input exceeds its allocation allowance",
        ));
    }
    // Admit complete source geometry/metadata before any encoded-copy allocation.
    let _ = inspect_with_limits(bytes, limits, check)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| resource("JPEG encoded copy allocation refused"))?;
    for chunk in bytes.chunks(65_536) {
        check()?;
        copy.extend_from_slice(chunk);
    }
    let source = Inspector::new(bytes, limits, check, Some(&mut copy)).run()?;
    Ok((source, copy))
}
impl<'a> Inspector<'a> {
    fn new(
        bytes: &'a [u8],
        limits: Limits,
        check: &'a dyn Fn() -> Result<()>,
        projection: Option<&'a mut [u8]>,
    ) -> Self {
        Self {
            bytes,
            limits,
            check,
            projection,
            frame: None,
            width: 0,
            height: 0,
            components: [Component::default(); 4],
            count: 0,
            adobe: None,
            density: None,
            exif: None,
            metadata: Metadata::default(),
            icc: [None; 255],
            icc_count: None,
            scans: 0,
        }
    }
    fn run(mut self) -> Result<Source> {
        let mut at = 2;
        loop {
            (self.check)()?;
            if self.bytes.get(at) != Some(&255) {
                return Err(malformed("JPEG marker prefix is missing"));
            }
            while self.bytes.get(at) == Some(&255) {
                at += 1;
                if at.is_multiple_of(65_536) {
                    (self.check)()?;
                }
            }
            let marker_at = at;
            let marker = *self
                .bytes
                .get(at)
                .ok_or_else(|| malformed("JPEG marker is truncated"))?;
            at += 1;
            match marker {
                0 | 0xd8 => return Err(malformed("JPEG marker is invalid or duplicates SOI")),
                0xd9 => {
                    if self.frame.is_none() || self.scans == 0 {
                        return Err(malformed("JPEG image frame or scan is missing"));
                    }
                    self.metadata.trailing_bytes = (self.bytes.len() - at) as u64;
                    break;
                }
                0x01 | 0xd0..=0xd7 => continue,
                _ => {}
            }
            let length = self
                .bytes
                .get(at..at + 2)
                .ok_or_else(|| malformed("JPEG segment length is truncated"))?;
            let length = usize::from(u16::from_be_bytes(length.try_into().unwrap()));
            if length < 2 {
                return Err(malformed("JPEG segment length is below two"));
            }
            let end = at
                .checked_add(length)
                .ok_or_else(|| malformed("JPEG segment length overflows"))?;
            let data = self
                .bytes
                .get(at + 2..end)
                .ok_or_else(|| malformed("JPEG segment extends outside the source"))?;
            match marker {
                0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
                    self.frame(marker, data)?
                }
                0xda => {
                    self.scan(data)?;
                    at = self.entropy_end(end)?;
                    continue;
                }
                0xe0..=0xef | 0xfe => {
                    self.metadata(marker, at + 2, data)?;
                    if matches!(marker, 0xe1 | 0xe2 | 0xed)
                        && let Some(view) = self.projection.as_deref_mut()
                    {
                        // COM's pinned decoder path only skips the declared
                        // payload. Lengths, payload bytes and offsets stay exact.
                        view[marker_at] = 0xfe;
                    }
                }
                _ => {}
            }
            at = end;
        }
        let model = self.model()?;
        let model_origin = if self.adobe.is_some() {
            ModelOrigin::Adobe
        } else if self.density.is_some() {
            ModelOrigin::Jfif
        } else if model == Model::Rgb {
            ModelOrigin::ComponentIds
        } else {
            ModelOrigin::ComponentCountConvention
        };
        if let Some(count) = self.icc_count {
            self.metadata.icc_segments = count;
            let mut digest = Sha256::new();
            for part in &self.icc[..usize::from(count)] {
                let (start, length) =
                    part.ok_or_else(|| malformed("JPEG ICC sequence is incomplete"))?;
                for chunk in self.bytes[start..start + length].chunks(65_536) {
                    (self.check)()?;
                    digest.update(chunk);
                }
            }
            self.metadata.icc_sha256 = digest.finalize().into();
        }
        Ok(Source {
            width: self.width,
            height: self.height,
            components: self.components,
            component_count: self.count,
            model,
            model_origin,
            frame: self.frame.unwrap(),
            orientation: self.exif.unwrap_or(1),
            adobe_transform: self.adobe,
            density: self.density,
            metadata: self.metadata,
            source_bytes: self.bytes.len() as u64,
            source_sha256: hash(self.bytes, self.check)?,
        })
    }
    fn entropy_end(&self, mut at: usize) -> Result<usize> {
        let mut checkpoint = at;
        while at < self.bytes.len() {
            if at - checkpoint >= 65_536 {
                (self.check)()?;
                checkpoint = at;
            }
            if self.bytes[at] != 255 {
                at += 1;
                continue;
            }
            let start = at;
            while self.bytes.get(at) == Some(&255) {
                at += 1;
                if at.is_multiple_of(65_536) {
                    (self.check)()?;
                }
            }
            match self.bytes.get(at) {
                Some(0 | 0xd0..=0xd7) => at += 1,
                Some(_) => return Ok(start),
                None => return Err(malformed("JPEG entropy marker is truncated")),
            }
        }
        Err(malformed("JPEG EOI marker is missing"))
    }
    fn frame(&mut self, marker: u8, data: &[u8]) -> Result<()> {
        if self.frame.is_some() || data.len() < 6 {
            return Err(malformed("JPEG frame is duplicate or truncated"));
        }
        let frame = match marker {
            0xc0 => Frame::Baseline,
            0xc1 => Frame::ExtendedSequential,
            0xc2 => Frame::Progressive,
            _ => return Err(unsupported("JPEG coding mode is not qualified")),
        };
        if data[0] != 8 {
            return Err(unsupported(
                "JPEG sample precision is not qualified by the selected decoder",
            ));
        }
        self.height = u32::from(u16::from_be_bytes([data[1], data[2]]));
        self.width = u32::from(u16::from_be_bytes([data[3], data[4]]));
        self.count = data[5];
        if self.width == 0
            || self.height == 0
            || !(1..=4).contains(&self.count)
            || data.len() != 6 + usize::from(self.count) * 3
        {
            return Err(malformed(
                "JPEG frame geometry or component table is invalid",
            ));
        }
        if u64::from(self.width) * u64::from(self.height)
            > self.limits.pixels.min(super::super::DECODE_PIXELS_MAX)
        {
            return Err(resource("JPEG frame exceeds its pixel allowance"));
        }
        for (index, data) in data[6..].as_chunks::<3>().0.iter().enumerate() {
            if self.components[..index].iter().any(|v| v.id == data[0]) {
                return Err(malformed("JPEG component identifiers are duplicate"));
            }
            let (horizontal, vertical) = (data[1] >> 4, data[1] & 15);
            if !(1..=4).contains(&horizontal) || !(1..=4).contains(&vertical) {
                return Err(unsupported(
                    "JPEG sampling factors are outside their qualified range",
                ));
            }
            self.components[index] = Component {
                id: data[0],
                horizontal,
                vertical,
                quantization_table: data[2],
            };
        }
        self.frame = Some(frame);
        Ok(())
    }
    fn scan(&mut self, data: &[u8]) -> Result<()> {
        if self.frame.is_none()
            || data.is_empty()
            || data[0] == 0
            || data[0] > self.count
            || data.len() != 1 + usize::from(data[0]) * 2 + 3
        {
            return Err(malformed("JPEG scan component table is invalid"));
        }
        for component in data[1..1 + usize::from(data[0]) * 2].as_chunks::<2>().0 {
            if !self.components[..usize::from(self.count)]
                .iter()
                .any(|v| v.id == component[0])
            {
                return Err(malformed("JPEG scan references an absent component"));
            }
        }
        self.scans += 1;
        Ok(())
    }
    fn metadata(&mut self, marker: u8, start: usize, data: &[u8]) -> Result<()> {
        self.metadata.bytes += data.len() as u64;
        if self.metadata.bytes > self.limits.metadata_bytes {
            return Err(resource("JPEG metadata exceeds caller allowance"));
        }
        match marker {
            0xe0 if data.starts_with(b"JFIF\0") => {
                if data.len() < 14 {
                    return Err(malformed("JPEG JFIF marker is truncated"));
                }
                let density = Density {
                    units: data[7],
                    x: u16::from_be_bytes([data[8], data[9]]),
                    y: u16::from_be_bytes([data[10], data[11]]),
                };
                if self.density.is_some_and(|v| v != density) {
                    return Err(unsupported("JPEG JFIF density declarations conflict"));
                }
                self.density = Some(density);
                let thumbnail = u64::from(data[12]) * u64::from(data[13]) * 3;
                if thumbnail > (data.len() - 14) as u64 {
                    return Err(malformed("JPEG JFIF thumbnail is truncated"));
                }
                self.metadata.jfif_thumbnail_bytes += (data.len() - 14) as u64;
            }
            0xee if data.starts_with(b"Adobe") => {
                if data.len() < 12 {
                    return Err(malformed("JPEG Adobe APP14 marker is truncated"));
                }
                let transform = data[11];
                if transform > 2 || self.adobe.is_some_and(|v| v != transform) {
                    return Err(unsupported(
                        "JPEG Adobe transform is unknown or conflicting",
                    ));
                }
                self.adobe = Some(transform);
            }
            0xe1 if data.starts_with(b"Exif\0\0") => {
                if self.exif.is_some() {
                    return Err(unsupported("JPEG has multiple EXIF primary declarations"));
                }
                self.exif = Some(super::exif::orientation(&data[6..], self.check)?);
                self.metadata.exif_bytes += (data.len() - 6) as u64;
            }
            0xe2 if data.starts_with(b"ICC_PROFILE\0") => {
                if data.len() < 14 || data[12] == 0 || data[13] == 0 || data[12] > data[13] {
                    return Err(malformed("JPEG ICC sequence header is invalid"));
                }
                if self.icc_count.is_some_and(|v| v != data[13])
                    || self.icc[usize::from(data[12] - 1)].is_some()
                {
                    return Err(malformed("JPEG ICC sequence is duplicate or conflicting"));
                }
                self.icc_count = Some(data[13]);
                self.icc[usize::from(data[12] - 1)] = Some((start + 14, data.len() - 14));
                self.metadata.icc_bytes += (data.len() - 14) as u64;
            }
            0xe1 if data.starts_with(b"http://ns.adobe.com/xap/1.0/\0")
                || data.starts_with(b"http://ns.adobe.com/xmp/extension/\0") =>
            {
                self.metadata.xmp_bytes += data.len() as u64
            }
            0xed => {
                self.metadata.iptc_bytes += data.len() as u64;
                self.metadata.iptc_segments += 1;
            }
            0xfe => {
                self.metadata.comment_bytes += data.len() as u64;
                self.metadata.comment_segments += 1;
            }
            _ => {
                self.metadata.other_bytes += data.len() as u64;
                self.metadata.other_segments += 1;
            }
        }
        Ok(())
    }
    fn model(&self) -> Result<Model> {
        let rgb_ids = self.count == 3 && self.components[..3].iter().map(|v| v.id).eq(*b"RGB");
        let model = match (self.count, self.adobe) {
            (1, None | Some(0)) => Model::Gray,
            (3, Some(0)) => Model::Rgb,
            (3, None) if rgb_ids => Model::Rgb,
            (3, None | Some(1)) if !rgb_ids => Model::YCbCr,
            (4, None | Some(0)) => Model::Cmyk {
                adobe_inverted: self.adobe.is_some(),
            },
            (4, Some(2)) => Model::Ycck,
            _ => {
                return Err(unsupported(
                    "JPEG component count, IDs and Adobe transform conflict",
                ));
            }
        };
        if self.density.is_some() && !matches!(model, Model::Gray | Model::YCbCr) {
            return Err(unsupported("JPEG JFIF and encoded color model conflict"));
        }
        Ok(model)
    }
}
