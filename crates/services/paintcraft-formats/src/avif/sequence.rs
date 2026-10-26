//! Bounded AV1 low-overhead/reduced-still admission, not a pixel decoder.
//! AOM AV1 section 5 sequence/color/frame syntax; broader streams fail closed.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence {
    pub width: u32,
    pub height: u32,
    pub profile: u8,
    pub level: u8,
    pub bit_depth: u8,
    pub monochrome: bool,
    pub subsampling_x: bool,
    pub subsampling_y: bool,
    pub chroma_position: u8,
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub full_range: bool,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
struct Bits<'a> {
    bytes: &'a [u8],
    bit: usize,
}
impl Bits<'_> {
    fn read(&mut self, count: usize) -> Result<u32> {
        let mut value = 0;
        for _ in 0..count {
            let byte = self
                .bytes
                .get(self.bit / 8)
                .ok_or_else(|| invalid("Truncated AV1 sequence/frame prefix"))?;
            value = (value << 1) | u32::from((byte >> (7 - self.bit % 8)) & 1);
            self.bit += 1;
        }
        Ok(value)
    }
    fn flag(&mut self) -> Result<bool> {
        Ok(self.read(1)? != 0)
    }
}
fn sequence(bytes: &[u8], pixels: u64) -> Result<Sequence> {
    let mut b = Bits { bytes, bit: 0 };
    let profile = b.read(3)? as u8;
    if profile > 2 || !b.flag()? || !b.flag()? {
        return Err(unsupported("AV1 requires a reduced still-picture sequence"));
    }
    let level = b.read(5)? as u8;
    if level > 23 && level != 31 {
        return Err(unsupported("Reserved AV1 sequence level"));
    }
    let width_bits = b.read(4)? as usize + 1;
    let height_bits = b.read(4)? as usize + 1;
    let width = b.read(width_bits)? + 1;
    let height = b.read(height_bits)? + 1;
    if u64::from(width) * u64::from(height) > pixels.min(INSPECT_PIXELS_MAX) {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "AV1 coded dimensions exceed allowance",
        ));
    }
    b.read(3)?; // superblock, filter-intra, intra-edge-filter
    if b.flag()? {
        return Err(unsupported("AV1 super-resolution is unqualified"));
    }
    b.read(2)?; // CDEF and restoration do not change sample geometry/color.
    let high = b.flag()?;
    let bit_depth = if profile == 2 && high {
        if b.flag()? { 12 } else { 10 }
    } else if high {
        10
    } else {
        8
    };
    let monochrome = profile != 1 && b.flag()?;
    let (primaries, transfer, matrix) = if b.flag()? {
        (b.read(8)? as u8, b.read(8)? as u8, b.read(8)? as u8)
    } else {
        (2, 2, 2)
    };
    let (full_range, subsampling_x, subsampling_y, chroma_position) = if monochrome {
        (b.flag()?, true, true, 0)
    } else {
        let (range, x, y) = if (primaries, transfer, matrix) == (1, 13, 0) {
            // AV1 profile/color conformance, not merely syntax inference:
            // profile 0 is 4:2:0; professional 4:4:4 requires 12 bits.
            if profile != 1 && !(profile == 2 && bit_depth == 12) {
                return Err(invalid(
                    "AV1 identity RGB conflicts with sequence profile/depth",
                ));
            }
            (true, false, false)
        } else {
            let range = b.flag()?;
            let (x, y) = match profile {
                0 => (true, true),
                1 => (false, false),
                _ if bit_depth == 12 => {
                    let x = b.flag()?;
                    (x, x && b.flag()?)
                }
                _ => (true, false),
            };
            (range, x, y)
        };
        let position = if x && y { b.read(2)? as u8 } else { 0 };
        b.flag()?; // separate_uv_delta_q
        (range, x, y, position)
    };
    if b.flag()? {
        return Err(unsupported("AV1 film grain is unqualified"));
    }
    if !b.flag()? {
        return Err(invalid("Missing AV1 sequence trailing-one bit"));
    }
    while b.bit < bytes.len() * 8 {
        if b.flag()? {
            return Err(invalid("Nonzero AV1 sequence trailing padding"));
        }
    }
    Ok(Sequence {
        width,
        height,
        profile,
        level,
        bit_depth,
        monochrome,
        subsampling_x,
        subsampling_y,
        chroma_position,
        primaries,
        transfer,
        matrix,
        full_range,
    })
}
struct Parts<'a> {
    values: &'a [&'a [u8]],
    part: usize,
    offset: usize,
    remaining: usize,
}
impl Parts<'_> {
    fn byte(&mut self) -> Result<u8> {
        while self.part < self.values.len() && self.offset == self.values[self.part].len() {
            self.part += 1;
            self.offset = 0;
        }
        let value = self
            .values
            .get(self.part)
            .and_then(|p| p.get(self.offset))
            .copied()
            .ok_or_else(|| invalid("Truncated AV1 OBU"))?;
        self.offset += 1;
        self.remaining -= 1;
        Ok(value)
    }
    fn skip(&mut self, mut count: usize) -> Result<()> {
        if count > self.remaining {
            return Err(invalid("AV1 OBU exceeds coded item"));
        }
        self.remaining -= count;
        while count > 0 {
            let available = self.values[self.part].len() - self.offset;
            let n = available.min(count);
            self.offset += n;
            count -= n;
            if self.offset == self.values[self.part].len() {
                self.part += 1;
                self.offset = 0;
            }
        }
        Ok(())
    }
    fn length(&mut self) -> Result<usize> {
        let mut value = 0u64;
        for index in 0..8 {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                if value > u64::from(u32::MAX) {
                    break;
                }
                return usize::try_from(value).map_err(|_| invalid("AV1 OBU length overflow"));
            }
        }
        Err(invalid("Invalid AV1 OBU LEB128 length"))
    }
}
/// Inspect all finite item extents without concatenating them or allocating
/// decoder scratch. Exactly one sequence and one combined frame OBU are required.
/// The reduced header fixes frame size to its sequence maximum; super-resolution
/// and differing render size are refused. Pixel/tile validation remains decoder work.
pub fn inspect_sequence(
    parts: &[&[u8]],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Sequence> {
    check()?;
    if parts.len() > 4096 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "AV1 coded extents exceed allowance",
        ));
    }
    let length = parts
        .iter()
        .try_fold(0usize, |n, p| n.checked_add(p.len()))
        .ok_or_else(|| invalid("AV1 coded extent overflow"))?;
    if length > INPUT_BYTES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "AV1 coded input exceeds allowance",
        ));
    }
    let mut input = Parts {
        values: parts,
        part: 0,
        offset: 0,
        remaining: length,
    };
    let mut result = None;
    let mut frame = false;
    let mut count = 0;
    while input.remaining > 0 {
        check()?;
        count += 1;
        if count > 4096 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "AV1 OBU count exceeds allowance",
            ));
        }
        let header = input.byte()?;
        if header & 0x85 != 0 || header & 2 == 0 {
            return Err(unsupported(
                "AV1 OBU forbidden/extension/reserved/size flags",
            ));
        }
        let kind = (header >> 3) & 15;
        let size = input.length()?;
        if size > input.remaining {
            return Err(invalid("AV1 OBU exceeds coded item"));
        }
        match kind {
            1 => {
                if result.is_some() || frame {
                    return Err(invalid("AV1 item has repeated/late sequence"));
                }
                if size > 4096 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "AV1 sequence exceeds allowance",
                    ));
                }
                let mut data = [0; 4096];
                for byte in &mut data[..size] {
                    *byte = input.byte()?;
                }
                result = Some(sequence(&data[..size], pixels)?);
            }
            6 => {
                if result.is_none() || frame || size == 0 {
                    return Err(invalid("AV1 item requires one frame after its sequence"));
                }
                let first = input.byte()?;
                let mut bits = Bits {
                    bytes: &[first],
                    bit: 0,
                };
                bits.flag()?; // disable_cdf_update
                if bits.flag()? {
                    bits.flag()?;
                } // screen tools / force_integer_mv
                if bits.flag()? {
                    return Err(unsupported(
                        "AV1 differing render dimensions are unqualified",
                    ));
                }
                input.skip(size - 1)?;
                frame = true;
            }
            2 if size == 0 && !frame => {}
            15 => input.skip(size)?,
            _ => return Err(unsupported("AV1 item OBU type is unqualified")),
        }
    }
    check()?;
    if !frame {
        return Err(invalid("AV1 item lacks a combined frame OBU"));
    }
    result.ok_or_else(|| invalid("AV1 item lacks a sequence"))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Writer(Vec<bool>);
    impl Writer {
        fn bits(&mut self, value: u32, count: usize) {
            self.0
                .extend((0..count).rev().map(|bit| value & (1 << bit) != 0));
        }
        fn bytes(mut self) -> Vec<u8> {
            self.0.push(true); // sequence trailing-one bit
            while !self.0.len().is_multiple_of(8) {
                self.0.push(false);
            }
            self.0
                .chunks(8)
                .map(|part| part.iter().fold(0, |v, bit| (v << 1) | u8::from(*bit)))
                .collect()
        }
    }
    // Declaration-only synthetic stream; no claim these tile bytes decode.
    fn synthetic(mono: bool, width: u32, height: u32, superres: bool, grain: bool) -> Vec<u8> {
        synthetic_profile(
            if mono { 0 } else { 1 },
            mono,
            width,
            height,
            superres,
            grain,
        )
    }
    fn synthetic_profile(
        profile: u32,
        mono: bool,
        width: u32,
        height: u32,
        superres: bool,
        grain: bool,
    ) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.bits(profile, 3);
        w.bits(3, 2); // still + reduced
        w.bits(0, 5); // level
        w.bits(15, 4);
        w.bits(15, 4);
        w.bits(width - 1, 16);
        w.bits(height - 1, 16);
        w.bits(0, 3);
        w.bits(u32::from(superres), 1);
        w.bits(0, 2);
        w.bits(0, 1); // high depth
        if profile != 1 {
            w.bits(u32::from(mono), 1);
        }
        w.bits(u32::from(!mono), 1);
        if mono {
            w.bits(1, 1);
        } else {
            w.bits(1, 8);
            w.bits(13, 8);
            w.bits(0, 8);
            w.bits(0, 1); // separate UV delta
        }
        w.bits(u32::from(grain), 1);
        let payload = w.bytes();
        let mut out = vec![0x0a, payload.len() as u8];
        out.extend(payload);
        out.extend([0x32, 1, 0]);
        out
    }
    #[test]
    fn identity_color_must_conform_to_profile_and_extent_count_precedes_scanning() {
        for profile in [0, 2] {
            let data = synthetic_profile(profile, false, 16, 16, false, false);
            assert_eq!(
                inspect_sequence(&[&data], 256, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CodecFailed
            );
        }
        // Professional 8-bit monochrome is conformant AV1 syntax. The later
        // strict image profile may refuse it; do not invent a normative ban.
        let data = synthetic_profile(2, true, 16, 16, false, false);
        assert_eq!(
            inspect_sequence(&[&data], 256, &|| Ok(())).unwrap().profile,
            2
        );
        let parts = vec![&[][..]; 4097];
        assert_eq!(
            inspect_sequence(&parts, 256, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    #[test]
    fn monochrome_alpha_declarations_and_geometry_are_bounded() {
        let data = synthetic(true, 31, 7, false, false);
        let seq = inspect_sequence(&[&data], 217, &|| Ok(())).unwrap();
        assert_eq!(
            (seq.width, seq.height, seq.monochrome, seq.full_range),
            (31, 7, true, true)
        );
        assert_eq!((seq.primaries, seq.transfer, seq.matrix), (2, 2, 2));
        let excessive = synthetic(false, 65536, 65536, false, false);
        assert_eq!(
            inspect_sequence(&[&excessive], u64::MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    #[test]
    fn unsupported_geometry_grain_and_obu_types_refuse_before_decode() {
        for data in [
            synthetic(false, 16, 16, true, false),
            synthetic(false, 16, 16, false, true),
        ] {
            assert_eq!(
                inspect_sequence(&[&data], 256, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
        }
        let mut data = synthetic(false, 16, 16, false, false);
        *data.last_mut().unwrap() = 0x20; // render_and_frame_size_different after two flags
        assert_eq!(
            inspect_sequence(&[&data], 256, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        for header in [0x8a, 0x0e, 0x0b, 0x08, 0x2a] {
            let mut malformed = synthetic(false, 16, 16, false, false);
            malformed[0] = header;
            assert!(inspect_sequence(&[&malformed], 256, &|| Ok(())).is_err());
        }
    }
    #[test]
    fn repeated_frames_sequences_truncated_and_overflowing_obus_fail_closed() {
        let data = synthetic(false, 16, 16, false, false);
        let mut duplicate = data.clone();
        duplicate.extend(&data);
        assert!(inspect_sequence(&[&duplicate], 256, &|| Ok(())).is_err());
        let mut duplicate = data.clone();
        duplicate.extend([0x32, 1, 0]);
        assert!(inspect_sequence(&[&duplicate], 256, &|| Ok(())).is_err());
        for data in [
            vec![0x0a, 0x80],
            vec![0x0a, 0xff, 0xff, 0xff, 0xff, 0x7f],
            vec![0x0a, 0],
            vec![0x0a, 1, 0],
        ] {
            assert!(inspect_sequence(&[&data], 256, &|| Ok(())).is_err());
        }
        for offset in 0..data.len() {
            let mut mutated = data.clone();
            mutated[offset] ^= 0xff;
            let _ = inspect_sequence(&[&mutated], 256, &|| Ok(()));
        }
    }
    #[test]
    fn selected_writer_coded_sequence_is_bounded_and_explicit_srgb() {
        let encoded =
            encode_rgba8(16, 16, &[80, 90, 100, 255].repeat(256), 100, &|| Ok(())).unwrap();
        let tree = paintcraft_paint::inspect_with_layout(
            &encoded.bytes,
            &paintcraft_paint::Limits::default(),
            &|| Ok(()),
            &crate::item_graph::layout,
        )
        .unwrap();
        let coded = tree
            .boxes
            .iter()
            .find(|b| b.kind == *b"mdat")
            .unwrap()
            .payload;
        let value = inspect_sequence(&[coded], 256, &|| Ok(())).unwrap();
        assert_eq!((value.width, value.height, value.bit_depth), (16, 16, 8));
        assert_eq!(
            (
                value.primaries,
                value.transfer,
                value.matrix,
                value.full_range
            ),
            (1, 13, 0, true)
        );
        assert!(!value.monochrome);
        let split: Vec<_> = coded.chunks(1).collect();
        assert_eq!(inspect_sequence(&split, 256, &|| Ok(())).unwrap(), value);
        assert_eq!(
            inspect_sequence(&[coded], 255, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        for n in 0..coded.len() {
            assert!(inspect_sequence(&[&coded[..n]], 256, &|| Ok(())).is_err());
        }
        assert_eq!(
            inspect_sequence(&[coded], 256, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel"
            )))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
}
