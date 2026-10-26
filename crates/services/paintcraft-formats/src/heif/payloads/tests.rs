use super::*;
type Entry = (u16, u16, u16, Vec<(u32, u32)>);
use crate::heif::tests::{bx, config, extent, fb, file, item, links};
pub(in crate::heif) fn stored(
    base: &[u8],
    entries: &[Entry],
    data: &[u8],
    in_file: bool,
) -> Vec<u8> {
    fn make(entries: &[Entry], origin: u32) -> Vec<u8> {
        let mut b = vec![0x44, 0];
        b.extend((entries.len() as u16).to_be_bytes());
        for (id, method, reference, parts) in entries {
            b.extend(id.to_be_bytes());
            b.extend(method.to_be_bytes());
            b.extend(reference.to_be_bytes());
            b.extend((parts.len() as u16).to_be_bytes());
            for (offset, len) in parts {
                b.extend((offset + origin).to_be_bytes());
                b.extend(len.to_be_bytes());
            }
        }
        fb(b"iloc", 1, 0, &b)
    }
    let n = u32::from_be_bytes(base[..4].try_into().unwrap()) as usize;
    let body = &base[n + 12..];
    let mut metadata = body.to_vec();
    metadata.extend(make(entries, 0));
    if !in_file {
        metadata.extend(bx(b"idat", data));
    }
    let mut out = base[..n].to_vec();
    let mut meta = fb(b"meta", 0, 0, &metadata);
    if in_file {
        let origin = (out.len() + meta.len() + 8) as u32;
        metadata = body.to_vec();
        metadata.extend(make(entries, origin));
        meta = fb(b"meta", 0, 0, &metadata);
    }
    out.extend(meta);
    if in_file {
        out.extend(bx(b"mdat", data));
    }
    out
}
fn base() -> Vec<u8> {
    file(
        1,
        &[item(1, b"hvc1", false)],
        &[extent(2, 2), config()],
        &[(1, vec![1, 2])],
        &[],
    )
}
#[test]
fn file_and_idat_payloads_are_borrowed_ordered_and_confined_to_storage() {
    for in_file in [false, true] {
        let method = u16::from(!in_file);
        let bytes = stored(
            &base(),
            &[(1, method, 0, vec![(2, 2), (0, 2)])],
            b"abcd",
            in_file,
        );
        let got = inspect_checked(&bytes, 4, &|| Ok(())).unwrap();
        let data = got.primary().unwrap().payload.as_ref().unwrap();
        assert_eq!(data.parts, [b"cd".as_slice(), b"ab".as_slice()]);
        assert_eq!(data.byte_len, 4);
        assert!(data.grid.is_none());
    }
}
#[test]
fn external_methods_bad_extents_and_duplicate_item_locations_fail_closed() {
    for entry in [
        (1, 1, 1, vec![(0, 1)]),
        (1, 2, 0, vec![(0, 1)]),
        (1, 1, 0, vec![(3, 2)]),
        (1, 1, 0, vec![(0, 0)]),
        (1, 0, 0, vec![(0, 1)]),
    ] {
        assert!(
            inspect_checked(&stored(&base(), &[entry], b"abcd", false), 4, &|| Ok(())).is_err()
        );
    }
    let entries = [(1, 1, 0, vec![(0, 1)]), (1, 1, 0, vec![(1, 1)])];
    assert!(inspect_checked(&stored(&base(), &entries, b"abcd", false), 4, &|| Ok(())).is_err());
    assert!(inspect_checked(&stored(&base(), &[], b"abcd", false), 4, &|| Ok(())).is_err());
    let entries = [(1, 1, 0, vec![(0, 3), (2, 2)])];
    assert!(inspect_checked(&stored(&base(), &entries, b"abcd", false), 4, &|| Ok(())).is_err());
}
#[test]
fn checked_grid_descriptor_matches_tile_order_geometry_and_partial_edge_crop() {
    let meta = file(
        1,
        &[
            item(1, b"grid", false),
            item(2, b"hvc1", true),
            item(3, b"hvc1", true),
        ],
        &[extent(5, 2), extent(3, 2), config()],
        &[(1, vec![1]), (2, vec![2, 3]), (3, vec![2, 3])],
        &[links(b"dimg", 1, &[3, 2])],
    );
    let entries = [
        (1, 1, 0, vec![(0, 8)]),
        (2, 1, 0, vec![(8, 2)]),
        (3, 1, 0, vec![(8, 2)]),
    ];
    let mut data = vec![0, 0, 0, 1, 0, 5, 0, 2];
    data.extend(b"ab");
    let bytes = stored(&meta, &entries, &data, false);
    let got = inspect_checked(&bytes, 10, &|| Ok(())).unwrap();
    assert_eq!(got.references(*b"dimg", 1), Some([3, 2].as_slice()));
    assert_eq!(
        got.primary().unwrap().payload.as_ref().unwrap().grid,
        Some(Grid {
            rows: 1,
            columns: 2,
            width: 5,
            height: 2
        })
    );
    for (index, value) in [(0, 1), (1, 2), (2, 1), (3, 0), (5, 4), (7, 3)] {
        let mut bad = data.clone();
        bad[index] = value;
        assert!(
            inspect_checked(&stored(&meta, &entries, &bad, false), 10, &|| Ok(())).is_err(),
            "descriptor index{index}"
        );
    }
}
#[test]
fn payload_resolution_honors_cancellation_without_returning_partial_items() {
    use std::cell::Cell;
    let bytes = stored(&base(), &[(1, 1, 0, vec![(0, 2), (2, 2)])], b"abcd", false);
    let calls = Cell::new(0);
    inspect_checked(&bytes, 4, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let error = inspect_checked(&bytes, 4, &|| {
            count.set(count.get() + 1);
            if count.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "stop"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(count.get(), stop);
    }
}

#[test]
fn version_zero_file_offsets_and_version_two_wide_ids_keep_their_own_field_widths() {
    let original = base();
    let prefix = u32::from_be_bytes(original[..4].try_into().unwrap()) as usize;
    let children = &original[prefix + 12..];
    let build = |offset: u32| {
        let mut body = vec![0x44, 0, 0, 1, 0, 1, 0, 0, 0, 1];
        body.extend(offset.to_be_bytes());
        body.extend(4u32.to_be_bytes());
        let mut meta = children.to_vec();
        meta.extend(fb(b"iloc", 0, 0, &body));
        let mut out = original[..prefix].to_vec();
        out.extend(fb(b"meta", 0, 0, &meta));
        out.extend(bx(b"mdat", b"abcd"));
        out
    };
    let template = build(0);
    let bytes = build((template.len() - 4) as u32);
    let got = inspect_checked(&bytes, 4, &|| Ok(())).unwrap();
    assert_eq!(
        got.primary().unwrap().payload.as_ref().unwrap().parts,
        [b"abcd".as_slice()]
    );
    let id = 65537u32;
    let mut item_body = id.to_be_bytes().to_vec();
    item_body.extend([0, 0]);
    item_body.extend(b"hvc1wide\0");
    let mut inventory = 1u32.to_be_bytes().to_vec();
    inventory.extend(fb(b"infe", 3, 0, &item_body));
    let mut iprp = bx(b"ipco", &[extent(2, 2), config()].concat());
    let mut associations = 1u32.to_be_bytes().to_vec();
    associations.extend(id.to_be_bytes());
    associations.extend([2, 0x80, 1, 0x80, 2]);
    iprp.extend(fb(b"ipma", 1, 1, &associations));
    let mut location = vec![0x44, 0];
    location.extend(1u32.to_be_bytes());
    location.extend(id.to_be_bytes());
    location.extend([0, 1, 0, 0, 0, 1]);
    location.extend(0u32.to_be_bytes());
    location.extend(4u32.to_be_bytes());
    let mut meta = fb(b"pitm", 1, 0, &id.to_be_bytes());
    meta.extend(fb(b"iinf", 1, 0, &inventory));
    meta.extend(bx(b"iprp", &iprp));
    meta.extend(fb(b"iloc", 2, 0, &location));
    meta.extend(bx(b"idat", b"wide"));
    let mut bytes = bx(b"ftyp", b"heic\0\0\0\0mif1");
    bytes.extend(fb(b"meta", 0, 0, &meta));
    let got = inspect_checked(&bytes, 4, &|| Ok(())).unwrap();
    assert_eq!(got.primary_id, id);
    assert_eq!(
        got.primary().unwrap().payload.as_ref().unwrap().parts,
        [b"wide".as_slice()]
    );
    assert!(
        got.primary()
            .unwrap()
            .properties
            .iter()
            .all(|p| p.essential)
    );
}
