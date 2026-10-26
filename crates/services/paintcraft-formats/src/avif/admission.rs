//! Internal isolated-worker admission. This does not enable a frontend reader.
use super::{properties::Property, *};
use crate::item_graph::{Item, invalid, unsupported};

pub const ALPHA_AUXILIARY_TYPE: &str = "urn:mpeg:mpegB:cicp:systems:auxiliary:alpha";
#[derive(Debug)]
pub struct CodedItem<'a> {
    pub id: u32,
    pub name: &'a str,
    pub width: u32,
    pub height: u32,
    pub sequence: Sequence,
    /// Borrowed finite iloc extents, in declared coded order. A worker may
    /// concatenate only under its already-retained input/heap reservation.
    pub parts: Vec<&'a [u8]>,
    pub byte_len: usize,
}
#[derive(Debug)]
pub struct Admitted<'a> {
    pub primary: CodedItem<'a>,
    pub alpha: Option<CodedItem<'a>>,
}
#[derive(Clone, Copy)]
pub(super) enum ColorPolicy {
    Identity,
    Ycbcr709,
    Family,
}
fn once<T>(slot: &mut Option<T>, value: T) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(invalid("Duplicate AVIF associated semantic property"));
    }
    Ok(())
}
fn coded<'a>(
    item: Item<'a, Property<'a>>,
    alpha: bool,
    policy: ColorPolicy,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<CodedItem<'a>> {
    check()?;
    if item.kind != *b"av01" {
        return Err(unsupported("AVIF metadata/derived items are unqualified"));
    }
    let mut spatial = None;
    let mut config = None;
    let mut bits = None;
    let mut color = None;
    let mut auxiliary = None;
    for association in item.properties {
        check()?;
        match association.property {
            Property::Spatial { width, height } => once(&mut spatial, (width, height))?,
            Property::Pixels(value) => once(&mut bits, value)?,
            Property::Av1(value) => {
                if !association.essential {
                    return Err(unsupported("AVIF av1C must be essential in this profile"));
                }
                once(&mut config, value)?;
            }
            Property::Nclx {
                primaries,
                transfer,
                matrix,
                full_range,
            } => once(&mut color, (primaries, transfer, matrix, full_range))?,
            Property::Auxiliary { kind, subtype } => once(&mut auxiliary, (kind, subtype))?,
            Property::Unsupported(_kind) => {
                return Err(unsupported(
                    "AVIF transforms/other associated properties are unqualified",
                ));
            }
        }
    }
    let (width, height) =
        spatial.ok_or_else(|| invalid("AVIF item lacks associated spatial extent"))?;
    let config = config.ok_or_else(|| invalid("AVIF item lacks associated AV1 configuration"))?;
    let bits = bits.ok_or_else(|| invalid("AVIF item lacks associated pixel information"))?;
    if bits != if alpha { &[8][..] } else { &[8, 8, 8][..] } {
        return Err(unsupported(
            "AVIF initial profile requires explicit 8-bit channels",
        ));
    }
    let payload = item
        .payload
        .ok_or_else(|| invalid("AVIF item lacks finite coded storage"))?;
    let sequence = inspect_sequence(&payload.parts, pixels, check)?;
    if (sequence.width, sequence.height) != (width, height)
        || config.profile != sequence.profile
        || config.level != sequence.level
        || config.tier
        || config.bit_depth != sequence.bit_depth
        || config.monochrome != sequence.monochrome
        || config.subsampling_x != sequence.subsampling_x
        || config.subsampling_y != sequence.subsampling_y
        || config.chroma_position != sequence.chroma_position
    {
        return Err(invalid(
            "AVIF associated properties disagree with coded sequence",
        ));
    }
    if sequence.bit_depth != 8 {
        return Err(unsupported("AVIF high-depth conversion is unqualified"));
    }
    if alpha {
        if auxiliary != Some((ALPHA_AUXILIARY_TYPE, &[][..]))
            || color.is_some()
            || sequence.profile != 0
            || !sequence.monochrome
            || !sequence.full_range
            || !sequence.subsampling_x
            || !sequence.subsampling_y
            || sequence.chroma_position != 0
            || (sequence.primaries, sequence.transfer, sequence.matrix) != (2, 2, 2)
        {
            return Err(unsupported(
                "AVIF requires an explicit full-range straight 8-bit alpha auxiliary",
            ));
        }
    } else {
        let selected = match policy {
            ColorPolicy::Family => match sequence.matrix {
                0 => ColorPolicy::Identity,
                1 => ColorPolicy::Ycbcr709,
                _ => return Err(unsupported("AVIF matrix has no qualified family profile")),
            },
            other => other,
        };
        let accepted = match selected {
            ColorPolicy::Identity => {
                color == Some((1, 13, 0, true))
                    && sequence.profile == 1
                    && !sequence.monochrome
                    && sequence.full_range
                    && !sequence.subsampling_x
                    && !sequence.subsampling_y
                    && sequence.chroma_position == 0
                    && (sequence.primaries, sequence.transfer, sequence.matrix) == (1, 13, 0)
            }
            ColorPolicy::Family => unreachable!("family selection resolved above"),
            ColorPolicy::Ycbcr709 => {
                let geometry = if sequence.subsampling_x && sequence.subsampling_y {
                    sequence.profile == 0 && matches!(sequence.chroma_position, 1 | 2)
                } else {
                    sequence.profile == 1
                        && !sequence.subsampling_x
                        && !sequence.subsampling_y
                        && sequence.chroma_position == 0
                };
                geometry
                    && !sequence.monochrome
                    && color == Some((1, 13, 1, sequence.full_range))
                    && (sequence.primaries, sequence.transfer, sequence.matrix) == (1, 13, 1)
            }
        };
        if auxiliary.is_some() || !accepted {
            return Err(unsupported(
                "AVIF color does not match the selected explicit profile",
            ));
        }
    }
    Ok(CodedItem {
        id: item.id,
        name: item.name,
        width,
        height,
        sequence,
        parts: payload.parts,
        byte_len: payload.byte_len,
    })
}
/// Admit direct reduced-still identity sRGB8 color plus optional full-range
/// monochrome straight alpha. Grid/derived items, transforms, metadata, ICC/HDR,
/// premultiplication and other color layouts remain explicit refusals. All item
/// extents and actual sequence/frame-prefix bounds are validated before a native
/// decoder allocation. This is only for a separately bounded isolated worker;
/// tile validation and postdecode agreement remain mandatory there.
pub fn admit_srgb8<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Admitted<'a>> {
    admit_with_policy(bytes, pixels, check, ColorPolicy::Identity)
}
pub(super) fn admit_with_policy<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
    policy: ColorPolicy,
) -> Result<Admitted<'a>> {
    let graph = graph::inspect(bytes, pixels, check)?;
    if !(1..=2).contains(&graph.items.len()) {
        return Err(unsupported(
            "AVIF additional items require a separate conversion policy",
        ));
    }
    let alpha_id = graph
        .items
        .iter()
        .find(|item| item.id != graph.primary_id)
        .map(|item| item.id);
    match alpha_id {
        Some(id) => {
            if graph.references.len() != 1
                || graph.references[0].kind != *b"auxl"
                || graph.references[0].from != id
                || graph.references[0].to != [graph.primary_id]
            {
                return Err(unsupported(
                    "AVIF requires one unambiguous straight alpha-to-primary link",
                ));
            }
        }
        None if !graph.references.is_empty() => {
            return Err(unsupported("AVIF extra item references are unqualified"));
        }
        None => {}
    }
    let mut primary = None;
    let mut alpha = None;
    for item in graph.items {
        check()?;
        let is_alpha = item.id != graph.primary_id;
        let item = coded(item, is_alpha, policy, pixels, check)?;
        if is_alpha {
            alpha = Some(item);
        } else {
            primary = Some(item);
        }
    }
    let primary = primary.ok_or_else(|| invalid("AVIF primary item vanished"))?;
    if alpha
        .as_ref()
        .is_some_and(|item| (item.width, item.height) != (primary.width, primary.height))
    {
        return Err(unsupported(
            "AVIF alpha must match primary geometry exactly",
        ));
    }
    check()?;
    Ok(Admitted { primary, alpha })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn encoded(alpha: bool) -> Vec<u8> {
        let rgba: Vec<u8> = (0..256)
            .flat_map(|n| [80, 90, 100, if alpha { n as u8 } else { 255 }])
            .collect();
        encode_rgba8(16, 16, &rgba, 100, &|| Ok(())).unwrap().bytes
    }
    fn position(bytes: &[u8], tag: &[u8]) -> usize {
        bytes.windows(tag.len()).position(|b| b == tag).unwrap() + tag.len()
    }
    #[test]
    fn current_writer_primary_and_straight_auxiliary_are_real_admitted_items() {
        for alpha in [false, true] {
            let bytes = encoded(alpha);
            let captured = bytes.clone();
            let admitted = admit_srgb8(&bytes, 256, &|| Ok(())).unwrap();
            assert_eq!((admitted.primary.width, admitted.primary.height), (16, 16));
            assert_eq!(admitted.primary.id, 1);
            assert_eq!(
                admitted.primary.byte_len,
                admitted
                    .primary
                    .parts
                    .iter()
                    .map(|p| p.len())
                    .sum::<usize>()
            );
            assert_eq!(admitted.alpha.is_some(), alpha);
            if let Some(item) = admitted.alpha {
                assert_eq!((item.id, item.width, item.height), (2, 16, 16));
                assert!(item.sequence.monochrome);
                assert!(item.sequence.full_range);
                assert_eq!(
                    (
                        item.sequence.primaries,
                        item.sequence.transfer,
                        item.sequence.matrix
                    ),
                    (2, 2, 2)
                );
            }
            assert_eq!(bytes, captured);
        }
    }
    #[test]
    fn independently_reopened_fixtures_agree_with_primary_and_alpha_declarations() {
        // FFprobe8/libdav1d identifies High/gbrp/pc/bt709/iec61966-2-1
        // and Main/gray/pc respectively; checked-in ffprobe.json is raw evidence.
        for (bytes, alpha) in [
            (
                &include_bytes!("../../tests/avif-fixtures/opaque.avif")[..],
                false,
            ),
            (
                &include_bytes!("../../tests/avif-fixtures/alpha.avif")[..],
                true,
            ),
        ] {
            let admitted = admit_srgb8(bytes, 256, &|| Ok(())).unwrap();
            let color = &admitted.primary.sequence;
            assert_eq!(color.level, 31); // actual av1C/reduced sequence index
            assert_eq!(
                (color.width, color.height, color.profile, color.bit_depth),
                (16, 16, 1, 8)
            );
            assert_eq!(
                (
                    color.primaries,
                    color.transfer,
                    color.matrix,
                    color.full_range
                ),
                (1, 13, 0, true)
            );
            assert_eq!(
                (
                    color.subsampling_x,
                    color.subsampling_y,
                    color.chroma_position
                ),
                (false, false, 0)
            );
            assert_eq!(admitted.alpha.is_some(), alpha);
            if let Some(alpha) = admitted.alpha {
                assert_eq!(
                    (
                        alpha.sequence.profile,
                        alpha.sequence.monochrome,
                        alpha.sequence.full_range
                    ),
                    (0, true, true)
                );
            }
        }
    }
    #[test]
    fn associated_sizes_depth_color_config_alpha_and_reference_mismatches_refuse() {
        let bytes = encoded(true);
        let mutations: [(&[u8], usize, u8); 10] = [
            (b"ispe", 7, 17),
            (b"av1C", 1, 0),
            (b"pixi", 5, 12),
            (b"colr", 7, 16),
            (b"auxC", 4 + ALPHA_AUXILIARY_TYPE.len() - 1, b'b'),
            (b"auxl", 1, 1),
            (b"auxl", 5, 2),
            (b"iloc", 0, 3),
            (b"iloc", 14, 255),
            (b"pitm", 5, 2),
        ];
        for (tag, offset, value) in mutations {
            let mut data = bytes.clone();
            let at = position(&data, tag) + offset;
            assert_ne!(data[at], value, "mutation must change {tag:?}");
            data[at] = value;
            assert!(
                admit_srgb8(&data, 256, &|| Ok(())).is_err(),
                "accepted {tag:?} mutation"
            );
        }
        let mut premultiplied = bytes.clone();
        let at = position(&premultiplied, b"auxl") - 4;
        premultiplied[at..at + 4].copy_from_slice(b"prem");
        assert!(admit_srgb8(&premultiplied, 256, &|| Ok(())).is_err());
    }
    #[test]
    fn unknown_essential_and_nonessential_transforms_are_never_ignored() {
        let bytes = encoded(false);
        for original in [b"av1C", b"pixi"] {
            for tag in [b"zzzz", b"irot", b"clap", b"imir"] {
                let mut data = bytes.clone();
                let at = position(&data, original) - 4;
                data[at..at + 4].copy_from_slice(tag);
                assert_eq!(
                    admit_srgb8(&data, 256, &|| Ok(())).unwrap_err().code,
                    ErrorCode::CapabilityUnsupported
                );
            }
        }
    }
    #[test]
    fn arbitrary_handler_and_unused_property_metadata_fail_closed() {
        let bytes = encoded(false);
        let mut wrong_handler = bytes.clone();
        let at = position(&wrong_handler, b"pict") - 4;
        wrong_handler[at..at + 4].copy_from_slice(b"vide");
        assert_eq!(
            admit_srgb8(&wrong_handler, 256, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let mut duplicate = bytes.clone();
        let handler = position(&duplicate, b"hdlr") - 8;
        let count =
            u32::from_be_bytes(duplicate[handler..handler + 4].try_into().unwrap()) as usize;
        let copy = duplicate[handler..handler + count].to_vec();
        let meta = position(&duplicate, b"meta") - 8;
        let size = u32::from_be_bytes(duplicate[meta..meta + 4].try_into().unwrap());
        duplicate[meta..meta + 4].copy_from_slice(&(size + count as u32).to_be_bytes());
        duplicate.splice(handler..handler, copy);
        let error = graph::inspect(&duplicate, 256, &|| Ok(())).unwrap_err();
        assert!(error.message.contains("duplicate"));
        let mut unassociated = bytes.clone();
        let at = position(&unassociated, b"pixi") - 4;
        unassociated[at..at + 4].copy_from_slice(b"zzzz");
        let at = position(&unassociated, b"ipma") + 13;
        assert_eq!(unassociated[at], 3);
        unassociated[at] = 0; // nonessential no-property association
        let error = graph::inspect(&unassociated, 256, &|| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("unassociated"));
    }
    #[test]
    fn opaque_extra_iprp_children_cannot_bypass_metadata_policy() {
        let mut bytes = encoded(false);
        let iprp = position(&bytes, b"iprp") - 8;
        let meta = position(&bytes, b"meta") - 8;
        let iprp_size = u32::from_be_bytes(bytes[iprp..iprp + 4].try_into().unwrap());
        let meta_size = u32::from_be_bytes(bytes[meta..meta + 4].try_into().unwrap());
        let end = iprp + iprp_size as usize;
        bytes[iprp..iprp + 4].copy_from_slice(&(iprp_size + 8).to_be_bytes());
        bytes[meta..meta + 4].copy_from_slice(&(meta_size + 8).to_be_bytes());
        bytes.splice(end..end, [0, 0, 0, 8, b'z', b'z', b'z', b'z']);
        let error = graph::inspect(&bytes, 256, &|| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("image property child"));
    }

    #[test]
    fn truncated_sources_budget_and_cancel_never_produce_an_admitted_result() {
        let bytes = encoded(true);
        for n in 0..bytes.len() {
            assert!(admit_srgb8(&bytes[..n], 256, &|| Ok(())).is_err());
        }
        assert_eq!(
            admit_srgb8(&bytes, 255, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        for stop in [1, 2, 5, 20] {
            let calls = Cell::new(0);
            let error = admit_srgb8(&bytes, 256, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
        }
    }
}
