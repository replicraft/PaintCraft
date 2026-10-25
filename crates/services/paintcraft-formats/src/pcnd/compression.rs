//! Bounded v1 Zstandard blocks. Wire framing is checked before decoder allocation.
use super::*;
use std::{borrow::Cow, io::Read};

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct Encoder(Option<zstd::bulk::Compressor<'static>>);
#[cfg(target_arch = "wasm32")]
pub(super) struct Encoder(bool);

impl Encoder {
    pub fn new(compact: bool) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let encoder = if compact {
                let mut encoder = zstd::bulk::Compressor::new(3)
                    .map_err(|_| codec("cannot create native block compressor"))?;
                for parameter in [
                    zstd::zstd_safe::CParameter::WindowLog(18),
                    zstd::zstd_safe::CParameter::ChecksumFlag(true),
                    zstd::zstd_safe::CParameter::ContentSizeFlag(true),
                ] {
                    encoder
                        .set_parameter(parameter)
                        .map_err(|_| codec("cannot configure native block compressor"))?;
                }
                Some(encoder)
            } else {
                None
            };
            Ok(Self(encoder))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Ok(Self(compact))
        }
    }
    pub fn block<'a>(&mut self, data: &'a [u8]) -> Result<(&'static str, Cow<'a, [u8]>)> {
        #[cfg(not(target_arch = "wasm32"))]
        let compact = self.0.is_some();
        #[cfg(target_arch = "wasm32")]
        let compact = self.0;
        let mut best = ("raw", Cow::Borrowed(data));
        if compact {
            let runs = super::runs::encode(data);
            if runs.len() < data.len() {
                best = ("rgba-runs", Cow::Owned(runs));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(encoder) = &mut self.0 {
            let compressed = encoder
                .compress(data)
                .map_err(|_| codec("native block compression failed"))?;
            if compressed.len() < best.1.len() {
                best = ("zstd", Cow::Owned(compressed));
            }
        }
        Ok(best)
    }
}

fn malformed() -> Error {
    codec("invalid bounded Zstandard block")
}

/// Accept one ordinary dictionary-free frame, with a window no larger than one
/// decoded tile. Scan all block envelopes and reject trailing/concatenated data
/// before initializing ruzstd, whose constructor otherwise allocates the window.
fn frame(bytes: &[u8], decoded: usize, check: &dyn Fn() -> Result<()>) -> Result<bool> {
    if decoded == 0
        || decoded > BLOCK_BYTES_MAX as usize
        || bytes.get(..4) != Some(&[0x28, 0xB5, 0x2F, 0xFD])
    {
        return Err(malformed());
    }
    let descriptor = *bytes.get(4).ok_or_else(malformed)?;
    if descriptor & 0x18 != 0 || descriptor & 3 != 0 {
        return Err(malformed());
    }
    let single = descriptor & 0x20 != 0;
    let checksum = descriptor & 4 != 0;
    let mut at = 5usize;
    let mut window = 0u64;
    if !single {
        let value = *bytes.get(at).ok_or_else(malformed)?;
        at += 1;
        let base = 1u64
            .checked_shl(10 + u32::from(value >> 3))
            .ok_or_else(malformed)?;
        window = base + base / 8 * u64::from(value & 7);
    }
    let size_bytes = match descriptor >> 6 {
        0 => usize::from(single),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    if size_bytes != 0 {
        let source = bytes.get(at..at + size_bytes).ok_or_else(malformed)?;
        let mut value = [0u8; 8];
        value[..size_bytes].copy_from_slice(source);
        let content = u64::from_le_bytes(value) + if size_bytes == 2 { 256 } else { 0 };
        if content != decoded as u64 {
            return Err(malformed());
        }
        if single {
            window = content;
        }
        at += size_bytes;
    }
    if window == 0 || window > BLOCK_BYTES_MAX {
        return Err(malformed());
    }
    loop {
        check()?;
        let header = bytes.get(at..at + 3).ok_or_else(malformed)?;
        let header = u32::from_le_bytes([header[0], header[1], header[2], 0]);
        at += 3;
        let length = (header >> 3) as usize;
        if length > 128 * 1024 {
            return Err(malformed());
        }
        let stored = match (header >> 1) & 3 {
            0 | 2 => length,
            1 => 1,
            _ => return Err(malformed()),
        };
        at = at
            .checked_add(stored)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(malformed)?;
        if header & 1 != 0 {
            break;
        }
    }
    if checksum {
        at = at.checked_add(4).ok_or_else(malformed)?;
    }
    if at != bytes.len() {
        return Err(malformed());
    }
    Ok(checksum)
}

pub(super) fn decode(
    bytes: &[u8],
    length: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let checksum = frame(bytes, length, check)?;
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes).map_err(|_| malformed())?;
    let mut output = Vec::new();
    output.try_reserve_exact(length).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "cannot allocate decoded native block",
        )
    })?;
    output.resize(length, 0);
    for chunk in output.chunks_mut(16 * 1024) {
        check()?;
        decoder.read_exact(chunk).map_err(|_| malformed())?;
    }
    check()?;
    let mut extra = [0];
    if decoder.read(&mut extra).map_err(|_| malformed())? != 0
        || !decoder.get_ref().is_empty()
        || !decoder.decoder.is_finished()
        || decoder.decoder.bytes_read_from_source() != bytes.len() as u64
        || (checksum
            && decoder.decoder.get_checksum_from_data()
                != decoder.decoder.get_calculated_checksum())
    {
        return Err(malformed());
    }
    check()?;
    Ok(output)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn encoded() -> (Vec<u8>, Vec<u8>) {
        let source = vec![71; 262_144];
        let mut encoder = zstd::bulk::Compressor::new(3).unwrap();
        encoder
            .set_parameter(zstd::zstd_safe::CParameter::ChecksumFlag(true))
            .unwrap();
        let bytes = encoder.compress(&source).unwrap();
        (source, bytes)
    }
    fn raw_frame(source: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x28, 0xB5, 0x2F, 0xFD, 0, 0x40];
        let header = ((source.len() as u32) << 3 | 1).to_le_bytes();
        bytes.extend_from_slice(&header[..3]);
        bytes.extend_from_slice(source);
        bytes
    }

    #[test]
    fn strict_decoder_rejects_all_truncated_frames_corruption_trailing_and_concatenation() {
        let (source, bytes) = encoded();
        assert_eq!(decode(&bytes, source.len(), &|| Ok(())).unwrap(), source);
        for cut in 0..bytes.len() {
            assert!(
                decode(&bytes[..cut], source.len(), &|| Ok(())).is_err(),
                "cut {cut}"
            );
        }
        let mut corrupted = bytes.clone();
        *corrupted.last_mut().unwrap() ^= 1;
        assert!(
            decode(&corrupted, source.len(), &|| Ok(())).is_err(),
            "frame checksum checked independently"
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&trailing, source.len(), &|| Ok(())).is_err());
        assert!(
            decode(
                &[bytes.clone(), bytes.clone()].concat(),
                source.len(),
                &|| Ok(())
            )
            .is_err()
        );
        assert!(decode(&bytes, source.len() - 4, &|| Ok(())).is_err());
        let mut dictionary = bytes;
        dictionary[4] |= 1;
        assert!(decode(&dictionary, source.len(), &|| Ok(())).is_err());
    }

    #[test]
    fn unknown_content_size_still_requires_exact_bounded_output_and_a_small_window() {
        let valid = raw_frame(&[1, 2, 3, 4]);
        assert_eq!(decode(&valid, 4, &|| Ok(())).unwrap(), [1, 2, 3, 4]);
        assert!(decode(&raw_frame(&[1, 2, 3]), 4, &|| Ok(())).is_err());
        assert!(decode(&raw_frame(&[1, 2, 3, 4, 5]), 4, &|| Ok(())).is_err());
        let mut oversized = valid.clone();
        oversized[5] = 0x48; // 512 KiB, before decoder window allocation.
        assert!(decode(&oversized, 4, &|| Ok(())).is_err());
        let mut reserved = valid.clone();
        reserved[6] |= 6;
        assert!(decode(&reserved, 4, &|| Ok(())).is_err());
        let mut skippable = valid;
        skippable[..4].copy_from_slice(&[0x50, 0x2A, 0x4D, 0x18]);
        assert!(decode(&skippable, 4, &|| Ok(())).is_err());
        assert!(decode(&raw_frame(&[]), 0, &|| Ok(())).is_err());
        assert!(decode(&raw_frame(&[]), BLOCK_BYTES_MAX as usize + 1, &|| Ok(())).is_err());
    }
}
