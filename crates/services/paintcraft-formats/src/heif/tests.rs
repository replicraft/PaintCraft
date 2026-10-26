//! Authored BMFF item metadata, not HEVC bitstreams or pixel-decoder evidence.
use super::*;
use std::cell::Cell;
pub(super) fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(payload);
    out
}
pub(super) fn fb(kind: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![
        version,
        ((flags >> 16) & 255) as u8,
        ((flags >> 8) & 255) as u8,
        (flags & 255) as u8,
    ];
    out.extend(payload);
    bx(kind, &out)
}
pub(super) fn extent(w: u32, h: u32) -> Vec<u8> {
    fb(b"ispe", 0, 0, &[w.to_be_bytes(), h.to_be_bytes()].concat())
}
pub(super) fn config() -> Vec<u8> {
    let mut out = vec![0; 23];
    out[0] = 1;
    out[16] = 1;
    out[21] = 3;
    out[22] = 1;
    out.extend([33, 0, 1, 0, 2, 66, 1]);
    bx(b"hvcC", &out)
}
pub(super) fn item(id: u16, kind: &[u8; 4], hidden: bool) -> Vec<u8> {
    let mut body = id.to_be_bytes().to_vec();
    body.extend([0, 0]);
    body.extend(kind);
    body.extend(format!("item-{id}\0").bytes());
    fb(b"infe", 2, u32::from(hidden), &body)
}
pub(super) fn links(kind: &[u8; 4], from: u16, to: &[u16]) -> Vec<u8> {
    let mut body = from.to_be_bytes().to_vec();
    body.extend((to.len() as u16).to_be_bytes());
    for id in to {
        body.extend(id.to_be_bytes());
    }
    bx(kind, &body)
}
pub(super) fn file(
    primary: u16,
    items: &[Vec<u8>],
    properties: &[Vec<u8>],
    associations: &[(u16, Vec<u8>)],
    references: &[Vec<u8>],
) -> Vec<u8> {
    let mut out = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
    let mut meta = fb(b"pitm", 0, 0, &primary.to_be_bytes());
    let mut inventory = (items.len() as u16).to_be_bytes().to_vec();
    for item in items {
        inventory.extend(item);
    }
    meta.extend(fb(b"iinf", 0, 0, &inventory));
    let mut props = Vec::new();
    for p in properties {
        props.extend(p);
    }
    let mut iprp = bx(b"ipco", &props);
    let mut assoc = (associations.len() as u32).to_be_bytes().to_vec();
    for (id, indices) in associations {
        assoc.extend(id.to_be_bytes());
        assoc.push(indices.len() as u8);
        assoc.extend(indices);
    }
    iprp.extend(fb(b"ipma", 0, 0, &assoc));
    meta.extend(bx(b"iprp", &iprp));
    if !references.is_empty() {
        let mut refs = Vec::new();
        for r in references {
            refs.extend(r);
        }
        meta.extend(fb(b"iref", 0, 0, &refs));
    }
    out.extend(fb(b"meta", 0, 0, &meta));
    out
}
fn single() -> Vec<u8> {
    file(
        7,
        &[item(2, b"hvc1", false), item(7, b"hvc1", false)],
        &[extent(3, 2), config(), fb(b"pixi", 0, 0, &[3, 8, 8, 8])],
        &[(2, vec![1, 2]), (7, vec![1, 2, 3])],
        &[],
    )
}
#[test]
fn explicit_primary_is_not_inventory_first_and_properties_are_borrowed_declarations() {
    let bytes = single();
    let got = inspect_checked(&bytes, 6, &|| Ok(())).unwrap();
    assert_eq!(got.primary_id, 7);
    assert_eq!(got.primary().unwrap().name, "item-7");
    assert_eq!(
        got.primary().unwrap().properties[2].property,
        Property::PixelInformation { bits: &[8, 8, 8] }
    );
    assert!(got.alpha().is_none());
    assert!(
        crate::registry::admit_open(
            crate::registry::Frontend::Studio,
            "image.heic",
            &bytes,
            crate::registry::Platform::Native
        )
        .is_err()
    );
}
#[test]
fn grid_order_alpha_direction_premultiplication_and_unknown_auxiliary_are_explicit() {
    let properties = [
        extent(6, 2),
        extent(3, 2),
        config(),
        fb(b"auxC", 0, 0, b"urn:mpeg:hevc:2015:auxid:1\0"),
        extent(6, 2),
        fb(b"auxC", 0, 0, b"urn:example:depth\0"),
    ];
    let items = [
        item(1, b"grid", false),
        item(2, b"hvc1", true),
        item(3, b"hvc1", true),
        item(4, b"hvc1", true),
        item(5, b"hvc1", true),
    ];
    let associations = [
        (1, vec![1]),
        (2, vec![2, 3]),
        (3, vec![2, 3]),
        (4, vec![3, 4, 5]),
        (5, vec![2, 3, 6]),
    ];
    let refs = [
        links(b"dimg", 1, &[3, 2]),
        links(b"auxl", 4, &[1]),
        links(b"prem", 1, &[4]),
        links(b"auxl", 5, &[1]),
    ];
    let bytes = file(1, &items, &properties, &associations, &refs);
    let got = inspect_checked(&bytes, 12, &|| Ok(())).unwrap();
    assert_eq!(got.references(*b"dimg", 1), Some([3, 2].as_slice()));
    assert_eq!(got.alpha().unwrap().id, 4);
    assert!(got.premultiplied_alpha());
    assert_eq!(got.items.len(), 5);
    let wrong = [
        links(b"dimg", 1, &[3, 2]),
        links(b"auxl", 1, &[4]),
        links(b"prem", 1, &[4]),
    ];
    assert!(
        inspect_checked(
            &file(1, &items, &properties, &associations, &wrong),
            12,
            &|| Ok(())
        )
        .is_err()
    );
    let missing = [links(b"dimg", 1, &[3, 99])];
    assert!(
        inspect_checked(
            &file(1, &items, &properties, &associations, &missing),
            12,
            &|| Ok(())
        )
        .is_err()
    );
    let duplicate = [links(b"dimg", 1, &[2, 2])];
    assert!(
        inspect_checked(
            &file(1, &items, &properties, &associations, &duplicate),
            12,
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn essential_properties_bad_indices_ambiguous_primary_and_derivation_cycles_fail_closed() {
    let items = [item(1, b"hvc1", false)];
    let props = [extent(1, 1), config(), bx(b"test", b"opaque")];
    assert!(
        inspect_checked(
            &file(1, &items, &props, &[(1, vec![1, 2, 0x83])], &[]),
            1,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        inspect_checked(
            &file(1, &items, &props, &[(1, vec![1, 2, 3])], &[]),
            1,
            &|| Ok(())
        )
        .is_ok()
    );
    for associations in [
        vec![(1, vec![1, 2, 4])],
        vec![(1, vec![1, 2, 2])],
        vec![(1, vec![1, 2, 0x80])],
        vec![(1, vec![1, 2]), (1, vec![1, 2])],
    ] {
        assert!(
            inspect_checked(&file(1, &items, &props, &associations, &[]), 1, &|| Ok(())).is_err()
        );
    }
    assert!(
        inspect_checked(
            &file(9, &items, &props, &[(1, vec![1, 2])], &[]),
            1,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        inspect_checked(
            &file(
                1,
                &[item(1, b"hvc1", true)],
                &props,
                &[(1, vec![1, 2])],
                &[]
            ),
            1,
            &|| Ok(())
        )
        .is_err()
    );
    let iden = [item(1, b"iden", false), item(2, b"iden", true)];
    assert!(
        inspect_checked(
            &file(
                1,
                &iden,
                &[extent(1, 1)],
                &[(1, vec![1]), (2, vec![1])],
                &[links(b"dimg", 1, &[2]), links(b"dimg", 2, &[1])]
            ),
            1,
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn color_depth_and_transform_order_are_not_silently_converted() {
    let mut nclx = b"nclx".to_vec();
    nclx.extend([0, 9, 0, 16, 0, 9, 128]);
    let props = [
        extent(2, 3),
        config(),
        fb(b"pixi", 0, 0, &[3, 10, 10, 10]),
        bx(b"colr", &nclx),
        bx(b"colr", b"profraw profile bytes"),
        bx(b"irot", &[1]),
        bx(b"imir", &[0]),
    ];
    let bytes = file(
        1,
        &[item(1, b"hvc1", false)],
        &props,
        &[(1, vec![1, 2, 3, 4, 5, 6, 7])],
        &[],
    );
    let got = inspect_checked(&bytes, 6, &|| Ok(())).unwrap();
    let p = &got.primary().unwrap().properties;
    assert_eq!(
        p[2].property,
        Property::PixelInformation {
            bits: &[10, 10, 10]
        }
    );
    assert_eq!(
        p[3].property,
        Property::Color(Color::Nclx {
            primaries: 9,
            transfer: 16,
            matrix: 9,
            full_range: true
        })
    );
    assert_eq!(
        p[4].property,
        Property::Color(Color::Icc {
            restricted: false,
            bytes: b"raw profile bytes"
        })
    );
    assert_eq!(
        p[5].property,
        Property::Rotation {
            quarter_turns_ccw: 1
        }
    );
    assert_eq!(p[6].property, Property::Mirror { axis: 0 });
    let bad = [extent(1, 1), config(), bx(b"irot", &[4])];
    assert!(
        inspect_checked(
            &file(
                1,
                &[item(1, b"hvc1", false)],
                &bad,
                &[(1, vec![1, 2, 3])],
                &[]
            ),
            1,
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn every_truncation_resource_budget_and_checkpoint_refuses_without_partial_inventory() {
    let bytes = single();
    for end in 0..bytes.len() {
        assert!(
            inspect_checked(&bytes[..end], 6, &|| Ok(())).is_err(),
            "prefix {end}"
        );
    }
    assert_eq!(
        inspect_checked(&bytes, 5, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let calls = Cell::new(0);
    inspect_checked(&bytes, 6, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let observed = Cell::new(0);
        let error = inspect_checked(&bytes, 6, &|| {
            observed.set(observed.get() + 1);
            if observed.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "stop"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(observed.get(), stop);
    }
}

#[test]
fn identity_primary_inherits_explicit_source_alpha_without_guessing_auxiliary_types() {
    let items = [
        item(1, b"iden", false),
        item(2, b"hvc1", true),
        item(3, b"hvc1", true),
    ];
    let props = [
        extent(1, 1),
        config(),
        fb(b"auxC", 0, 0, b"urn:mpeg:hevc:2015:auxid:1\0"),
    ];
    let assoc = [(1, vec![1]), (2, vec![1, 2]), (3, vec![1, 2, 3])];
    let refs = [
        links(b"dimg", 1, &[2]),
        links(b"auxl", 3, &[2]),
        links(b"prem", 2, &[3]),
    ];
    let bytes = file(1, &items, &props, &assoc, &refs);
    let got = inspect_checked(&bytes, 1, &|| Ok(())).unwrap();
    assert_eq!(got.alpha().unwrap().id, 3);
    assert!(got.premultiplied_alpha());
}
#[test]
fn duplicate_alpha_and_malformed_hevc_array_frames_are_refused_before_any_codec() {
    let items = [
        item(1, b"hvc1", false),
        item(2, b"hvc1", true),
        item(3, b"hvc1", true),
    ];
    let props = [
        extent(1, 1),
        config(),
        fb(b"auxC", 0, 0, b"urn:mpeg:hevc:2015:auxid:1\0"),
    ];
    let bytes = file(
        1,
        &items,
        &props,
        &[(1, vec![1, 2]), (2, vec![1, 2, 3]), (3, vec![1, 2, 3])],
        &[links(b"auxl", 2, &[1]), links(b"auxl", 3, &[1])],
    );
    assert!(inspect_checked(&bytes, 1, &|| Ok(())).is_err());
    for at in [8, 30, 31, 32, 33, 34] {
        let mut hvcc = config();
        hvcc[at] = 255;
        let props = [extent(1, 1), hvcc];
        let bytes = file(1, &items[..1], &props, &[(1, vec![1, 2])], &[]);
        assert!(
            inspect_checked(&bytes, 1, &|| Ok(())).is_err(),
            "hvcC offset {at}"
        );
    }
}

#[test]
fn clean_aperture_retains_rational_extents_and_signed_offsets() {
    let values = [3u32, 2, 5, 2, u32::MAX, 2, 1, 3];
    let raw: Vec<u8> = values.into_iter().flat_map(u32::to_be_bytes).collect();
    let props = [extent(3, 2), config(), bx(b"clap", &raw)];
    let bytes = file(
        1,
        &[item(1, b"hvc1", false)],
        &props,
        &[(1, vec![1, 2, 3])],
        &[],
    );
    let got = inspect_checked(&bytes, 6, &|| Ok(())).unwrap();
    assert_eq!(
        got.primary().unwrap().properties[2].property,
        Property::CleanAperture {
            width: (3, 2),
            height: (5, 2),
            horizontal_offset: (-1, 2),
            vertical_offset: (1, 3)
        }
    );
    let mut invalid = raw;
    invalid[4..8].fill(0);
    let props = [extent(3, 2), config(), bx(b"clap", &invalid)];
    assert!(
        inspect_checked(
            &file(
                1,
                &[item(1, b"hvc1", false)],
                &props,
                &[(1, vec![1, 2, 3])],
                &[]
            ),
            6,
            &|| Ok(())
        )
        .is_err()
    );
}
