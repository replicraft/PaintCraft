use super::*;
use facts::{ResolutionUnit, invalid, unsupported};
use flate2::read::ZlibDecoder;

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}
pub(super) fn inspect(
    bytes: &[u8],
    limit: u64,
    result: &mut Descriptor,
    omitted: &mut Sha256,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let mut at = 8usize;
    let mut seen = 0u32;
    let mut idat = false;
    let mut idat_ended = false;
    let mut palette_seen = false;
    let mut ended = false;
    while at < bytes.len() {
        check()?;
        let head = bytes
            .get(
                at..at
                    .checked_add(8)
                    .ok_or_else(|| invalid("PNG chunk offset"))?,
            )
            .ok_or_else(|| invalid("Truncated PNG chunk header"))?;
        let length = be32(&head[..4]) as usize;
        let kind: [u8; 4] = head[4..8].try_into().unwrap();
        if length > 0x7fff_ffff
            || !kind.iter().all(u8::is_ascii_alphabetic)
            || kind[2].is_ascii_lowercase()
        {
            return Err(invalid("Invalid PNG chunk length/type"));
        }
        let end = at
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| invalid("PNG chunk extent"))?;
        let chunk = bytes
            .get(at..end)
            .ok_or_else(|| invalid("Truncated PNG chunk"))?;
        let data = &chunk[8..8 + length];
        let mut crc = crc32fast::Hasher::new();
        for part in chunk[4..8 + length].chunks(65536) {
            check()?;
            crc.update(part);
        }
        if crc.finalize() != be32(&chunk[8 + length..]) {
            return Err(invalid("PNG chunk CRC mismatch"));
        }
        if ended {
            return Err(invalid("PNG data after IEND"));
        }
        if at == 8 && kind != *b"IHDR" {
            return Err(invalid("PNG missing initial IHDR"));
        }
        if idat && kind != *b"IDAT" {
            idat_ended = true;
        }
        if kind[0].is_ascii_lowercase() {
            result.omitted_ancillary_count += 1;
            result.omitted_ancillary_bytes += length as u64;
            facts::hash_update(omitted, chunk, check)?;
        }
        let slot = match &kind {
            b"IHDR" => Some(0),
            b"PLTE" => Some(1),
            b"tRNS" => Some(2),
            b"gAMA" => Some(3),
            b"cHRM" => Some(4),
            b"sRGB" => Some(5),
            b"iCCP" => Some(6),
            b"pHYs" => Some(7),
            b"sBIT" => Some(8),
            b"cICP" => Some(9),
            b"mDCV" => Some(10),
            b"cLLI" => Some(11),
            _ => None,
        };
        if let Some(slot) = slot {
            if seen & (1 << slot) != 0 {
                return Err(invalid("Duplicate PNG source declaration"));
            }
            seen |= 1 << slot;
            if idat {
                return Err(invalid("PNG source declaration follows IDAT"));
            }
            if palette_seen
                && matches!(
                    &kind,
                    b"gAMA" | b"cHRM" | b"sRGB" | b"iCCP" | b"sBIT" | b"cICP" | b"mDCV" | b"cLLI"
                )
            {
                return Err(invalid("PNG color declaration follows palette"));
            }
        }
        match &kind {
            b"IHDR" => {
                if at != 8 || data.len() != 13 {
                    return Err(invalid("PNG IHDR order/length"));
                }
            }
            b"PLTE" => {
                if data.is_empty()
                    || !data.len().is_multiple_of(3)
                    || data.len() > 768
                    || matches!(result.color_type, 0 | 4)
                    || result.color_type == 3 && data.len() / 3 > 1usize << result.bit_depth
                {
                    return Err(invalid("PNG palette domain"));
                }
                result.palette.rgb = data.as_chunks::<3>().0.to_vec();
                palette_seen = true;
            }
            b"tRNS" => {
                let valid = match result.color_type {
                    3 => palette_seen && !data.is_empty() && data.len() <= result.palette.rgb.len(),
                    0 => data.len() == 2,
                    2 => data.len() == 6,
                    _ => false,
                };
                if !valid {
                    return Err(invalid("PNG transparency domain"));
                }
                result.transparency = data.to_vec();
                if result.color_type == 3 {
                    result.palette.alpha = data.to_vec();
                }
            }
            b"gAMA" => {
                if data.len() != 4 || be32(data) == 0 {
                    return Err(invalid("PNG gamma declaration"));
                }
                result.gamma_scaled = Some(be32(data));
            }
            b"cHRM" => {
                if data.len() != 32 {
                    return Err(invalid("PNG chromaticity declaration"));
                }
                result.chromaticities_scaled =
                    Some(std::array::from_fn(|i| be32(&data[i * 4..i * 4 + 4])));
            }
            b"sRGB" => {
                if data.len() != 1 || data[0] > 3 {
                    return Err(invalid("PNG sRGB intent"));
                }
                result.srgb_intent = Some(data[0]);
            }
            b"cICP" => {
                if data.len() != 4 || data[3] > 1 {
                    return Err(invalid("PNG cICP declaration"));
                }
                result.cicp = Some(data.try_into().unwrap());
            }
            b"mDCV" => {
                result.mastering_display = Some(
                    data.try_into()
                        .map_err(|_| invalid("PNG mastering display declaration"))?,
                );
            }
            b"cLLI" => {
                result.content_light = Some(
                    data.try_into()
                        .map_err(|_| invalid("PNG content light declaration"))?,
                );
            }
            b"iCCP" => {
                let nul = data
                    .iter()
                    .position(|v| *v == 0)
                    .ok_or_else(|| invalid("PNG ICC profile name"))?;
                if !(1..=79).contains(&nul)
                    || data.get(nul + 1) != Some(&0)
                    || !data[..nul]
                        .iter()
                        .all(|v| matches!(v,0x20..=0x7e|0xa1..=0xff))
                    || data[0] == b' '
                    || data[nul - 1] == b' '
                    || data[..nul].windows(2).any(|w| w == b"  ")
                {
                    return Err(invalid("PNG ICC profile name/compression"));
                }
                let mut reader = ZlibDecoder::new(&data[nul + 2..]);
                let icc = facts::inspect_icc(&mut reader, limit, check)?;
                if reader.total_in() != (data.len() - nul - 2) as u64 {
                    return Err(invalid("PNG ICC trailing compressed data"));
                }
                let expected = if matches!(result.color_type, 0 | 4) {
                    *b"GRAY"
                } else {
                    *b"RGB "
                };
                if icc.color_space != expected {
                    return Err(invalid("PNG ICC source channel model mismatch"));
                }
                result.icc = Some(icc);
            }
            b"pHYs" => {
                if data.len() != 9 || data[8] > 1 {
                    return Err(invalid("PNG physical dimensions declaration"));
                }
                result.resolution = Some(Resolution {
                    x: be32(&data[..4]),
                    y: be32(&data[4..8]),
                    unit: if data[8] == 1 {
                        ResolutionUnit::Meter
                    } else {
                        ResolutionUnit::Unknown
                    },
                });
            }
            b"sBIT" => {
                let count = match result.color_type {
                    0 => 1,
                    2 | 3 => 3,
                    4 => 2,
                    6 => 4,
                    _ => unreachable!(),
                };
                if data.len() != count
                    || data.iter().any(|v| {
                        *v == 0
                            || *v
                                > if result.color_type == 3 {
                                    8
                                } else {
                                    result.bit_depth
                                }
                    })
                {
                    return Err(invalid("PNG significant sample bits"));
                }
                result.significant_bits = data.to_vec();
            }
            b"IDAT" => {
                if idat_ended {
                    return Err(invalid("Nonconsecutive PNG image data"));
                }
                if result.color_type == 3 && !palette_seen {
                    return Err(invalid("Indexed PNG lacks palette"));
                }
                idat = true;
            }
            b"IEND" => {
                if !data.is_empty() || !idat {
                    return Err(invalid("PNG IEND without image data"));
                }
                ended = true;
            }
            b"eXIf" => {
                if result.exif.is_some() {
                    return Err(invalid("Duplicate PNG EXIF declaration"));
                }
                result.exif = Some(facts::digest(data, check)?);
            }
            b"acTL" | b"fcTL" | b"fdAT" => {}
            _ if kind[0].is_ascii_lowercase() => result.other_metadata = true,
            _ => return Err(unsupported("Unknown PNG critical chunk")),
        }
        at = end;
    }
    if !ended {
        return Err(invalid("PNG missing IEND"));
    }
    check()
}
