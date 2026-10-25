use super::*;
use paintcraft_core::types::ErrorCode;
use std::cell::Cell;
fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(payload);
    bytes
}
fn ftyp() -> Vec<u8> {
    atom(b"ftyp", b"mif1\0\0\0\0mif1gcmiisoamiaf")
}
fn read(bytes: &[u8]) -> Result<BoxTree<'_>> {
    inspect(bytes, &Limits::default(), &|| Ok(()))
}
#[test]
fn observed_container_nesting_and_shared_brands_remain_structural_only() {
    let property = atom(b"ispe", &[0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 2]);
    let properties = atom(b"iprp", &atom(b"ipco", &property));
    let mut payload = vec![0; 4];
    payload.extend_from_slice(&properties);
    let mut bytes = ftyp();
    bytes.extend_from_slice(&atom(b"meta", &payload));
    bytes.extend_from_slice(&atom(b"mdat", &[1, 2, 3, 4]));
    let tree = read(&bytes).unwrap();
    assert_eq!(
        tree.boxes.iter().map(|b| b.kind).collect::<Vec<_>>(),
        [*b"ftyp", *b"meta", *b"iprp", *b"ipco", *b"ispe", *b"mdat"]
    );
    assert_eq!(
        tree.boxes.iter().map(|b| b.parent).collect::<Vec<_>>(),
        [None, None, Some(1), Some(2), Some(3), None]
    );
    assert_eq!(
        tree.boxes.iter().map(|b| b.depth).collect::<Vec<_>>(),
        [0, 0, 1, 2, 3, 0]
    );
    let brands = tree.file_type.unwrap();
    assert_eq!(brands.major_brand, *b"mif1");
    assert!(brands.has_brand(*b"miaf"));
    assert!(!brands.has_brand(*b"avif"));
    assert_eq!(tree.boxes[4].payload, &property[8..]);
    assert!(!tree.boxes[4].children_parsed);
    for node in &tree.boxes {
        assert_eq!(
            node.payload,
            &bytes[node.range.start + node.header_bytes..node.range.end]
        );
        if let Some(parent) = node.parent {
            let parent = &tree.boxes[parent];
            assert!(node.range.start >= parent.range.start + parent.header_bytes);
            assert!(node.range.end <= parent.range.end);
        }
    }
}
#[test]
fn extended_uuid_and_top_level_to_end_sizes_have_exact_headers() {
    let mut large = 1u32.to_be_bytes().to_vec();
    large.extend_from_slice(b"uuid");
    large.extend_from_slice(&36u64.to_be_bytes());
    large.extend_from_slice(&[7; 16]);
    large.extend_from_slice(&[1, 2, 3, 4]);
    let tree = read(&large).unwrap();
    assert_eq!(tree.boxes[0].header_bytes, 32);
    assert_eq!(tree.boxes[0].user_type, Some([7; 16]));
    assert_eq!(tree.boxes[0].payload, [1, 2, 3, 4]);
    let mut tail = vec![0, 0, 0, 0];
    tail.extend_from_slice(b"mdat");
    tail.extend_from_slice(&[1, 2, 3]);
    let tree = read(&tail).unwrap();
    assert_eq!(tree.boxes[0].range, 0..11);
    assert_eq!(tree.boxes[0].payload, [1, 2, 3]);
    let mut payload = vec![0; 4];
    payload.extend_from_slice(&tail);
    assert_eq!(
        read(&atom(b"meta", &payload)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn unknown_payload_is_opaque_even_if_it_looks_like_nested_boxes() {
    let bytes = atom(b"unkn", &[0, 0, 0, 1, b'j', b'u', b'n', b'k']);
    let tree = read(&bytes).unwrap();
    assert_eq!(tree.boxes.len(), 1);
    assert!(!tree.boxes[0].children_parsed);
}
#[test]
fn malformed_sizes_truncation_scope_and_full_box_versions_are_refused() {
    for declared in [2u32, 4, 7, 100] {
        let mut bytes = declared.to_be_bytes().to_vec();
        bytes.extend_from_slice(b"free");
        assert!(read(&bytes).is_err());
    }
    let mut large = 1u32.to_be_bytes().to_vec();
    large.extend_from_slice(b"free");
    large.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(read(&large).is_err());
    let bytes = atom(b"mdat", &[1, 2, 3, 4]);
    for end in 1..bytes.len() {
        assert!(
            read(&bytes[..end]).is_err(),
            "accepted truncated length {end}"
        );
    }
    let mut child = 100u32.to_be_bytes().to_vec();
    child.extend_from_slice(b"free");
    let mut parent = vec![0; 4];
    parent.extend_from_slice(&child);
    assert!(read(&atom(b"meta", &parent)).is_err());
    assert_eq!(
        read(&atom(b"meta", &[1, 0, 0, 0])).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(read(&atom(b"meta", &[0, 0])).is_err());
    let mut duplicates = ftyp();
    duplicates.extend_from_slice(&ftyp());
    assert!(read(&duplicates).is_err());
    assert!(read(&atom(b"ftyp", &[0; 9])).is_err());
}
#[test]
fn count_depth_metadata_source_and_brand_limits_are_enforced() {
    let mut bytes = ftyp();
    bytes.extend_from_slice(&atom(b"free", &[]));
    for limits in [
        Limits {
            source_bytes: bytes.len() - 1,
            ..Default::default()
        },
        Limits {
            boxes: 1,
            ..Default::default()
        },
        Limits {
            metadata_bytes: 1,
            ..Default::default()
        },
        Limits {
            compatible_brands: 1,
            ..Default::default()
        },
    ] {
        assert_eq!(
            inspect(&bytes, &limits, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    let mut bytes = atom(b"leaf", &[]);
    for _ in 0..66 {
        bytes = atom(b"cont", &bytes);
    }
    let layout = |b: &BoxInfo<'_>, _: Option<[u8; 4]>| {
        Ok(if b.kind == *b"cont" {
            Layout::Children { prefix_bytes: 0 }
        } else {
            Layout::Opaque
        })
    };
    assert_eq!(
        inspect_with_layout(
            &bytes,
            &Limits {
                depth: usize::MAX,
                ..Default::default()
            },
            &|| Ok(()),
            &layout
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn injected_layout_prefix_and_ancestry_cannot_escape_physical_ranges() {
    let bytes = atom(b"cont", &atom(b"leaf", &[42]));
    let layout = |b: &BoxInfo<'_>, parent: Option<[u8; 4]>| {
        if b.kind == *b"cont" {
            assert_eq!(parent, None);
            Ok(Layout::Children { prefix_bytes: 0 })
        } else {
            assert_eq!(parent, Some(*b"cont"));
            Ok(Layout::Opaque)
        }
    };
    let tree = inspect_with_layout(&bytes, &Limits::default(), &|| Ok(()), &layout).unwrap();
    assert_eq!(tree.boxes.len(), 2);
    assert_eq!(tree.boxes[1].payload, [42]);
    assert!(
        inspect_with_layout(&bytes, &Limits::default(), &|| Ok(()), &|_, _| Ok(
            Layout::Children {
                prefix_bytes: usize::MAX
            }
        ))
        .is_err()
    );
}
#[test]
fn file_relative_extents_require_bounds_and_explicit_alias_policy() {
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
    assert_eq!(
        resolve_extent(
            &bytes,
            2..8,
            Extent {
                offset: 3,
                length: 3
            }
        )
        .unwrap(),
        [4, 5, 6]
    );
    for extent in [
        Extent {
            offset: 1,
            length: 1,
        },
        Extent {
            offset: 7,
            length: 2,
        },
        Extent {
            offset: u64::MAX,
            length: 1,
        },
    ] {
        assert!(resolve_extent(&bytes, 2..8, extent).is_err());
    }
    assert!(
        resolve_extent(
            &bytes,
            0..9,
            Extent {
                offset: 0,
                length: 1
            }
        )
        .is_err()
    );
    let adjacent = [
        Extent {
            offset: 4,
            length: 4,
        },
        Extent {
            offset: 0,
            length: 4,
        },
    ];
    assert!(
        validate_extents(
            &adjacent,
            0..8,
            AliasPolicy::Reject,
            &Limits::default(),
            &|| Ok(())
        )
        .is_ok()
    );
    let aliases = [Extent {
        offset: 0,
        length: 4,
    }; 2];
    assert!(
        validate_extents(
            &aliases,
            0..8,
            AliasPolicy::Reject,
            &Limits::default(),
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        validate_extents(
            &aliases,
            0..8,
            AliasPolicy::AllowExact,
            &Limits::default(),
            &|| Ok(())
        )
        .is_ok()
    );
    let partial = [
        Extent {
            offset: 0,
            length: 5,
        },
        Extent {
            offset: 4,
            length: 4,
        },
    ];
    assert!(
        validate_extents(
            &partial,
            0..8,
            AliasPolicy::AllowExact,
            &Limits::default(),
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        validate_extents(
            &[],
            std::ops::Range { start: 8, end: 0 },
            AliasPolicy::Reject,
            &Limits::default(),
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        validate_extents(
            &aliases,
            0..8,
            AliasPolicy::AllowExact,
            &Limits {
                extents: 1,
                ..Default::default()
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn cancellation_propagates_without_mutation() {
    let bytes = ftyp();
    let frozen = bytes.clone();
    let n = Cell::new(0);
    let check = || {
        let count = n.get();
        n.set(count + 1);
        if count >= 1 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "test cancellation",
            ))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        inspect(&bytes, &Limits::default(), &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(bytes, frozen);
    assert_eq!(
        validate_extents(&[], 0..0, AliasPolicy::Reject, &Limits::default(), &|| Err(
            Error::new(ErrorCode::OperationCancelled, "cancel")
        ))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
