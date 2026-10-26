use super::*;
use std::cell::Cell;

const MIXED: &[u8] = include_bytes!("../../tests/abr-fixtures/samples-v6.abr");
const V10: &[u8] = include_bytes!("../../tests/abr-fixtures/samples-v10.abr");

fn section(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = b"\0\x06\0\x01".to_vec();
    bytes.extend(b"8BIM");
    bytes.extend(kind);
    bytes.extend((payload.len() as u32).to_be_bytes());
    bytes.extend(payload);
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    bytes
}
fn sample(bounds: [i32; 4], depth: u16, compression: u8, data: &[u8]) -> Vec<u8> {
    let mut body = b"\x01x".to_vec();
    body.extend([0; 10]);
    for value in bounds {
        body.extend(value.to_be_bytes());
    }
    body.extend(depth.to_be_bytes());
    body.push(compression);
    body.extend(data);
    let mut record = (body.len() as u32).to_be_bytes().to_vec();
    record.extend(body);
    record.resize(record.len().next_multiple_of(4), 0);
    section(b"samp", &record)
}
fn descriptor(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = 16u32.to_be_bytes().to_vec();
    bytes.extend(0u32.to_be_bytes()); // empty descriptor name
    bytes.extend(0u32.to_be_bytes()); // four-byte class ID
    bytes.extend(b"null");
    bytes.extend(1u32.to_be_bytes());
    bytes.extend(0u32.to_be_bytes());
    bytes.extend(b"test");
    bytes.extend(kind);
    bytes.extend(payload);
    bytes
}

#[test]
fn independent_samples_and_presets_keep_precision_origins_and_raw_metadata() {
    let library = decode(MIXED, &|| Ok(())).unwrap();
    assert_eq!(library.version(), (6, 1));
    assert_eq!(library.samples().len(), 3);
    assert_eq!(
        library.samples()[0],
        Sample {
            id: "raw-8".into(),
            bounds: [-2, -1, 0, 2],
            alpha: Alpha::U8(vec![0, 1, 127, 128, 254, 255])
        }
    );
    assert_eq!(
        library.samples()[1].alpha,
        Alpha::U8(vec![0, 1, 255, 127, 127, 127])
    );
    assert_eq!(
        library.samples()[2].alpha,
        Alpha::U16(vec![0, 1, 255, 256, 32769, 65535])
    );
    assert_eq!(library.brushes().len(), 2);
    assert_eq!(library.brushes()[0].name, "Owned computed 🚀");
    assert!(matches!(
        library.brushes()[0].shape,
        ag_psd::abr::BrushShape::Computed {
            size: 30.0,
            angle: 15.0,
            roundness: 0.8,
            hardness: 0.6,
            spacing: 0.25,
            spacing_on: true,
            ..
        }
    ));
    assert!(
        matches!(&library.brushes()[1].shape, ag_psd::abr::BrushShape::Sampled { sampled_data, .. } if sampled_data == "raw-8")
    );
    let mut pos = 4;
    for retained in library.sections() {
        let n = u32::from_be_bytes(MIXED[pos + 8..pos + 12].try_into().unwrap()) as usize;
        assert_eq!(retained.payload, &MIXED[pos + 12..pos + 12 + n]);
        pos += 12 + n.next_multiple_of(4);
    }
    assert_eq!(library.sections()[2].kind, SectionKind::Hierarchy);
}

#[test]
fn all_selected_major_and_minor_layouts_are_admitted() {
    for major in [6u16, 7, 9, 10] {
        for (minor, original) in [(1, MIXED), (2, V10)] {
            let mut bytes = original.to_vec();
            bytes[..2].copy_from_slice(&major.to_be_bytes());
            let parsed = decode(&bytes, &|| Ok(())).unwrap();
            assert_eq!(parsed.version(), (major, minor));
        }
    }
    assert!(
        decode(
            include_bytes!("../../tests/abr-fixtures/empty.abr"),
            &|| Ok(())
        )
        .unwrap()
        .samples()
        .is_empty()
    );
}

#[test]
fn all_nonboundary_truncations_and_appended_garbage_are_refused() {
    let mut ends = vec![4];
    let mut pos = 4;
    while pos < MIXED.len() {
        let n = u32::from_be_bytes(MIXED[pos + 8..pos + 12].try_into().unwrap()) as usize;
        pos += 12 + n.next_multiple_of(4);
        ends.push(pos);
    }
    for end in 0..MIXED.len() {
        if !ends.contains(&end) {
            assert!(decode(&MIXED[..end], &|| Ok(())).is_err(), "prefix {end}");
        }
    }
    let mut trailing = MIXED.to_vec();
    trailing.extend([0; 12]);
    assert!(decode(&trailing, &|| Ok(())).is_err());
}

#[test]
fn packbits_requires_complete_bounded_packets_and_exact_rows() {
    for data in [
        vec![0, 2, 0, 2, 3, 8, 254, 5],    // truncated literal packet
        vec![0, 2, 0, 2, 253, 8, 254, 5],  // run crosses row
        vec![0, 2, 0, 2, 255, 8, 254, 5],  // incomplete first row
        vec![0, 1, 0, 2, 254, 254, 5],     // missing repeat value
        vec![0, 4, 0, 2, 2, 1, 2, 3, 254], // truncated second row
    ] {
        assert!(decode(&sample([0, 0, 2, 3], 8, 1, &data), &|| Ok(())).is_err());
    }
    let data = [0, 5, 0, 3, 128, 2, 1, 2, 3, 128, 254, 5];
    assert_eq!(
        decode(&sample([0, 0, 2, 3], 8, 1, &data), &|| Ok(()))
            .unwrap()
            .samples()[0]
            .alpha,
        Alpha::U8(vec![1, 2, 3, 5, 5, 5])
    );
}

#[test]
fn dangerous_dimensions_depths_sections_and_counts_fail_before_backend() {
    for bounds in [
        [0, 0, i32::MAX, 1],
        [i32::MIN, 0, i32::MAX, 1],
        [0, 0, 16384, 16384],
        [1, 0, 0, 2],
    ] {
        assert!(decode(&sample(bounds, 8, 0, &[]), &|| Ok(())).is_err());
    }
    for (depth, compression) in [(1, 0), (32, 0), (16, 1), (8, 2)] {
        assert_eq!(
            decode(&sample([0, 0, 1, 1], depth, compression, &[0]), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        decode(&section(b"patt", &[]), &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(decode(&section(b"oops", &[]), &|| Ok(())).is_err());
    let mut crossing = sample([0, 0, 1, 1], 8, 0, &[0]);
    crossing[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode(&crossing, &|| Ok(())).is_err());
    assert_eq!(
        decode(&vec![0; BYTES_MAX + 1], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn descriptors_enforce_counts_depth_unicode_and_finite_values() {
    let malformed = [
        descriptor(b"VlLs", &u32::MAX.to_be_bytes()),
        descriptor(b"TEXT", &u32::MAX.to_be_bytes()),
        descriptor(b"TEXT", &[0, 0, 0, 1, 0xd8, 0]),
        descriptor(b"doub", &f64::NAN.to_bits().to_be_bytes()),
        descriptor(
            b"UnFl",
            &[b"#Pxl".as_slice(), &f32::INFINITY.to_bits().to_be_bytes()].concat(),
        ),
        descriptor(b"bool", &[2]),
        descriptor(b"????", &[]),
        descriptor(b"tdta", &u32::MAX.to_be_bytes()),
        descriptor(b"obj ", &u32::MAX.to_be_bytes()),
    ];
    for payload in malformed {
        assert!(decode(&section(b"desc", &payload), &|| Ok(())).is_err());
    }
    let mut nested = descriptor(b"long", &[0; 4]);
    for _ in 0..18 {
        nested = descriptor(b"Objc", &nested[4..]);
    }
    assert_eq!(
        decode(&section(b"phry", &nested), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn bounded_metadata_atoms_and_paths_are_retained_without_resource_effects() {
    let class = [
        0u32.to_be_bytes().as_slice(),
        0u32.to_be_bytes().as_slice(),
        b"null",
    ]
    .concat();
    let identifier = [0u32.to_be_bytes().as_slice(), b"test"].concat();
    let unicode = [1u32.to_be_bytes().as_slice(), &[0, b'x']].concat();
    let path_text = "never-open"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let path_size = 12 + path_text.len() as u32;
    let path = [
        path_size.to_be_bytes().as_slice(),
        b"txtu",
        path_size.to_le_bytes().as_slice(),
        10u32.to_le_bytes().as_slice(),
        &path_text,
    ]
    .concat();
    let object_array = [
        16u32.to_be_bytes().as_slice(),
        &class,
        1u32.to_be_bytes().as_slice(),
        &identifier,
        b"UnFl#Pxl",
        1u32.to_be_bytes().as_slice(),
        0.25f64.to_bits().to_be_bytes().as_slice(),
    ]
    .concat();
    let references = [
        2u32.to_be_bytes().as_slice(),
        b"Clss",
        &class,
        b"Idnt",
        42u32.to_be_bytes().as_slice(),
    ]
    .concat();
    for (kind, value) in [
        (*b"TEXT", unicode),
        (*b"enum", [&identifier[..], &identifier].concat()),
        (*b"comp", vec![0; 8]),
        (*b"type", class),
        (
            *b"alis",
            [10u32.to_be_bytes().as_slice(), b"never-open"].concat(),
        ),
        (*b"ObAr", object_array),
        (*b"obj ", references),
        (*b"Pth ", path),
        (
            *b"UnFl",
            [
                b"#Pxl".as_slice(),
                0.25f32.to_bits().to_be_bytes().as_slice(),
            ]
            .concat(),
        ),
    ] {
        let payload = descriptor(&kind, &value);
        let library = decode(&section(b"phry", &payload), &|| Ok(())).unwrap();
        assert_eq!(library.sections()[0].payload, payload);
        assert!(library.brushes().is_empty());
        assert!(library.samples().is_empty());
    }
}

#[test]
fn aggregate_sample_and_section_counts_are_not_just_per_record_limits() {
    let one = sample([0, 0, 1, 1], 8, 0, &[1]);
    let mut too_many = one.clone();
    for _ in 1..=SECTIONS_MAX {
        too_many.extend(&one[4..]);
    }
    assert_eq!(
        decode(&too_many, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let record = &one[16..];
    assert_eq!(
        decode(&section(b"samp", &record.repeat(SAMPLES_MAX + 1)), &|| Ok(
            ()
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn deterministic_mutations_never_panic_in_the_selected_codec() {
    for index in 0..MIXED.len() {
        for byte in [0, 0xff] {
            let mut bytes = MIXED.to_vec();
            bytes[index] = byte;
            let _ = decode(&bytes, &|| Ok(()));
        }
    }
}

#[test]
fn cancellation_at_every_checkpoint_returns_no_partial_library() {
    let total = Cell::new(0);
    decode(MIXED, &|| {
        total.set(total.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 0..total.get() {
        let calls = Cell::new(0);
        let result = decode(MIXED, &|| {
            let n = calls.get();
            calls.set(n + 1);
            if n == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint {stop}"
        );
    }
}
