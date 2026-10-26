//! Developer qualification only. No production decoder dispatch is added.
use super::payloads::tests::stored;
use super::tests::{bx, extent, fb, file, item, links};
use super::*;
use heic_decoder::hevc::{
    nal::parse_nal_header,
    params::{Sps, parse_sps},
};
const GRADIENT: &[u8] = include_bytes!("../../tests/heif-fixtures/gray32x24.h265");
const CONSTANT: &[u8] = include_bytes!("../../tests/heif-fixtures/constant32x24.h265");
fn nals(bytes: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i..].starts_with(&[0, 0, 1]) {
            starts.push((i, i + 3));
        }
    }
    starts
        .iter()
        .enumerate()
        .map(|(i, (_, start))| {
            let end = starts.get(i + 1).map_or(bytes.len(), |s| s.0);
            let data = &bytes[*start..end];
            let n = data.iter().rposition(|b| *b != 0).map_or(0, |p| p + 1);
            &data[..n]
        })
        .collect()
}
fn sps(nal: &[u8], pixels: u64) -> Result<Sps> {
    if nal.len() > 65535 {
        return Err(resource("SPS NAL exceeds qualification allowance"));
    }
    let (header, payload) = parse_nal_header(nal).map_err(|_| invalid("invalid SPS NAL"))?;
    if header.nal_unit_type != 33 || header.nuh_layer_id != 0 || header.nuh_temporal_id_plus1 != 1 {
        return Err(unsupported("qualification expects a base-layer SPS"));
    }
    let rbsp = heic_decoder::bitreader::strip_emulation_prevention(payload);
    // Source-audited bounded parameter parser, never the frame decoder. See
    // README's RPS/scaling/VUI/HRD audit; this is not allocator confinement.
    let sps = parse_sps(&rbsp).map_err(|_| invalid("candidate refused SPS"))?;
    if sps.chroma_format_idc > 3
        || !matches!(sps.bit_depth_luma, 8 | 10 | 12)
        || !matches!(sps.bit_depth_chroma, 8 | 10 | 12)
    {
        return Err(unsupported(
            "candidate SPS precision/chroma outside qualification",
        ));
    }
    if sps.pic_width_in_luma_samples > 16384
        || sps.pic_height_in_luma_samples > 16384
        || u64::from(sps.pic_width_in_luma_samples) * u64::from(sps.pic_height_in_luma_samples)
            > pixels.min(PIXELS_MAX)
    {
        return Err(resource(
            "coded SPS dimensions exceed qualification allowance",
        ));
    }
    Ok(sps)
}
fn config(bytes: &[u8]) -> Vec<u8> {
    let units = nals(bytes);
    let raw_sps = units.iter().find(|nal| (nal[0] >> 1) & 63 == 33).unwrap();
    let parsed = sps(raw_sps, PIXELS_MAX).unwrap();
    let rbsp = heic_decoder::bitreader::strip_emulation_prevention(&raw_sps[2..]);
    let mut header = vec![0; 23];
    header[0] = 1;
    header[1..13].copy_from_slice(&rbsp[1..13]);
    header[13] = 0xf0;
    header[15] = 0xfc;
    header[16] = 0xfc | parsed.chroma_format_idc as u8;
    header[17] = 0xf8 | (parsed.bit_depth_luma - 8) as u8;
    header[18] = 0xf8 | (parsed.bit_depth_chroma - 8) as u8;
    header[21] = 15;
    header[22] = 3;
    for kind in [32, 33, 34] {
        let nal = units.iter().find(|nal| (nal[0] >> 1) & 63 == kind).unwrap();
        header.push(128 | kind);
        header.extend(1u16.to_be_bytes());
        header.extend((nal.len() as u16).to_be_bytes());
        header.extend(*nal);
    }
    bx(b"hvcC", &header)
}
fn picture(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    for nal in nals(bytes)
        .into_iter()
        .filter(|nal| (nal[0] >> 1) & 63 < 32)
    {
        result.extend((nal.len() as u32).to_be_bytes());
        result.extend(nal);
    }
    result
}
fn validate_all(value: &Inspection<'_>, pixels: u64) -> Result<()> {
    let mut aggregate = 0u64;
    for item in value.items.iter().filter(|item| item.kind == *b"hvc1") {
        let (luma, chroma, format, bytes) = item
            .properties
            .iter()
            .find_map(|a| {
                if let Property::HevcConfig {
                    luma_bits,
                    chroma_bits,
                    chroma,
                    bytes,
                    ..
                } = a.property
                {
                    Some((luma_bits, chroma_bits, chroma, bytes))
                } else {
                    None
                }
            })
            .ok_or_else(|| invalid("missing configuration"))?;
        let mut r = metadata::Reader::new(bytes);
        let header = r.take(23)?;
        let mut found = false;
        let mut largest_coded = 0u64;
        for _ in 0..header[22] {
            let kind = r.u8()? & 63;
            let count = r.u16()?;
            for _ in 0..count {
                let n = usize::from(r.u16()?);
                let nal = r.take(n)?;
                if kind == 33 {
                    let s = sps(nal, pixels)?;
                    found = true;
                    largest_coded = largest_coded.max(
                        u64::from(s.pic_width_in_luma_samples)
                            * u64::from(s.pic_height_in_luma_samples),
                    );
                    if s.bit_depth_luma != u32::from(luma)
                        || s.bit_depth_chroma != u32::from(chroma)
                        || s.chroma_format_idc != u32::from(format)
                    {
                        return Err(invalid("hvcC declarations disagree with SPS"));
                    }
                    for a in &item.properties {
                        if let Property::PixelInformation { bits } = a.property {
                            let channels = if s.chroma_format_idc == 0 { 1 } else { 3 };
                            if bits.len() != channels
                                || u32::from(bits[0]) != s.bit_depth_luma
                                || bits[1..]
                                    .iter()
                                    .any(|b| u32::from(*b) != s.bit_depth_chroma)
                            {
                                return Err(invalid("pixi declarations disagree with SPS"));
                            }
                        }
                    }
                    let [left, right, top, bottom] = s.conformance_window;
                    let display = (
                        s.pic_width_in_luma_samples - left - right,
                        s.pic_height_in_luma_samples - top - bottom,
                    );
                    if !item.properties.iter().any(|a|matches!(a.property,Property::SpatialExtent{width,height} if (width,height)==display)){return Err(invalid("spatial extent disagrees with SPS conformance window"));}
                }
            }
        }
        if !found {
            return Err(invalid("no SPS in coded image"));
        }
        aggregate = aggregate
            .checked_add(largest_coded)
            .ok_or_else(|| resource("coded SPS aggregate overflow"))?;
        if aggregate > pixels.min(PIXELS_MAX).saturating_mul(4) {
            return Err(resource(
                "coded SPS aggregate exceeds qualification allowance",
            ));
        }
    }
    let format = |item: &Item<'_>| {
        item.properties.iter().find_map(|a| {
            if let Property::HevcConfig {
                luma_bits,
                chroma_bits,
                chroma,
                ..
            } = a.property
            {
                Some((luma_bits, chroma_bits, chroma))
            } else {
                None
            }
        })
    };
    for item in value.items.iter().filter(|item| item.kind == *b"grid") {
        let ids = value
            .references(*b"dimg", item.id)
            .ok_or_else(|| invalid("grid has no tile order"))?;
        let first = ids
            .first()
            .and_then(|id| value.item(*id))
            .and_then(format)
            .ok_or_else(|| invalid("grid lacks a coded tile"))?;
        if ids
            .iter()
            .any(|id| value.item(*id).and_then(format) != Some(first))
        {
            return Err(invalid("grid tiles have different coded depths/chroma"));
        }
    }
    Ok(())
}
fn source(grid: bool, alpha: bool) -> Vec<u8> {
    let gradient = picture(GRADIENT);
    let constant = picture(CONSTANT);
    let mut data = Vec::new();
    let mut props = vec![extent(32, 24), config(GRADIENT), fb(b"pixi", 0, 0, &[1, 8])];
    let mut items = vec![item(2, b"hvc1", true), item(7, b"hvc1", false)];
    let mut assoc = vec![(2, vec![1, 2, 3]), (7, vec![1, 2, 3])];
    let mut refs = Vec::new();
    let mut entries = Vec::new();
    data.extend(&constant);
    entries.push((2, 1, 0, vec![(0, constant.len() as u32)]));
    let offset = data.len() as u32;
    data.extend(&gradient);
    entries.push((7, 1, 0, vec![(offset, gradient.len() as u32)]));
    let primary = if grid {
        items[1] = item(7, b"hvc1", true);
        items.push(item(9, b"grid", false));
        props.push(extent(64, 24));
        assoc.push((9, vec![4]));
        refs.push(links(b"dimg", 9, &[7, 2]));
        let offset = data.len() as u32;
        data.extend([0, 0, 0, 1, 0, 64, 0, 24]);
        entries.push((9, 1, 0, vec![(offset, 8)]));
        9
    } else {
        7
    };
    if alpha {
        props.push(fb(b"auxC", 0, 0, b"urn:mpeg:hevc:2015:auxid:1\0"));
        assoc[0].1.push(props.len() as u8);
        refs.push(links(b"auxl", 2, &[primary]));
    }
    stored(
        &file(primary, &items, &props, &assoc, &refs),
        &entries,
        &data,
        false,
    )
}
#[test]
fn independent_encoder_parameter_sets_pass_every_coded_image_admission() {
    let original = GRADIENT.to_vec();
    let raw = nals(GRADIENT)
        .into_iter()
        .find(|nal| (nal[0] >> 1) & 63 == 33)
        .unwrap();
    let s = sps(raw, PIXELS_MAX).unwrap();
    assert_eq!(
        (s.pic_width_in_luma_samples, s.pic_height_in_luma_samples),
        (32, 24)
    );
    assert_eq!((s.bit_depth_luma, s.chroma_format_idc), (8, 0));
    assert_eq!(
        sps(raw, 32 * 24 - 1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(original, GRADIENT);
    let bytes = source(true, false);
    let inventory = inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
    validate_all(&inventory, PIXELS_MAX).unwrap();
    let mut smaller = inventory.clone();
    let tile = smaller.items.iter_mut().find(|item| item.id == 2).unwrap();
    let Property::SpatialExtent { width, .. } = &mut tile.properties[0].property else {
        panic!()
    };
    *width = 31;
    assert!(validate_all(&smaller, PIXELS_MAX).is_err());
    let mut wrong_depth = inventory.clone();
    wrong_depth.items[0].properties[2].property = Property::PixelInformation { bits: &[10] };
    assert!(validate_all(&wrong_depth, PIXELS_MAX).is_err());
    let mut amplified = inventory.clone();
    for id in 20..24 {
        let mut copy = inventory.items[0].clone();
        copy.id = id;
        amplified.items.push(copy);
    }
    assert_eq!(
        validate_all(&amplified, 32 * 24).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn trusted_primary_grid_and_alpha_decode_match_independent_gray_samples() {
    let gradient: Vec<i32> = (0..24)
        .flat_map(|y| (0..32).map(move |x| (x * 7 + y * 11) % 256))
        .collect();
    for (grid, alpha) in [(false, false), (true, false), (false, true)] {
        let bytes = source(grid, alpha);
        let inventory = inspect_checked(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        validate_all(&inventory, PIXELS_MAX).unwrap();
        let got = heic_decoder::decode(&bytes).unwrap();
        assert_eq!((got.width, got.height), (if grid { 64 } else { 32 }, 24));
        assert_eq!((got.bit_depth_luma, got.chroma_format_idc), (8, 0));
        let expected = if grid {
            (0..24)
                .flat_map(|y| {
                    gradient[y * 32..(y + 1) * 32]
                        .iter()
                        .copied()
                        .chain(std::iter::repeat_n(17, 32))
                })
                .collect::<Vec<_>>()
        } else {
            gradient.clone()
        };
        assert_eq!(got.y, expected);
        assert_eq!(got.alpha.is_some(), alpha);
        if let Some(alpha) = got.alpha {
            assert_eq!(alpha.bit_depth, 8);
            assert_eq!(alpha.samples, vec![17; 32 * 24]);
            assert!(!alpha.premultiplied);
        }
    }
}
