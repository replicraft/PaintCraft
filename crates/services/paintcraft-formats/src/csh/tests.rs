use super::*;
use std::cell::Cell;

// Deliberately authored wire data, not output from the selected codec's writer.
fn record(selector: u16, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() <= 24);
    let mut bytes = selector.to_be_bytes().to_vec();
    bytes.extend(payload);
    bytes.resize(26, 0);
    bytes
}
fn subpath(open: bool, count: u16) -> Vec<u8> {
    record(
        if open { 3 } else { 0 },
        &[count.to_be_bytes(), 1u16.to_be_bytes(), 2u16.to_be_bytes()].concat(),
    )
}
fn file(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"cush\0\0\0\x02\0\0\0\x01".to_vec();
    let name: Vec<_> = "形 🚀".encode_utf16().chain([0]).collect();
    bytes.extend((name.len() as u32).to_be_bytes());
    for value in name {
        bytes.extend(value.to_be_bytes());
    }
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    bytes.extend(1u32.to_be_bytes());
    let mut body = b"\x03abc".to_vec();
    for value in [10u32, 20, 110, 220] {
        body.extend(value.to_be_bytes());
    }
    for record in records {
        body.extend(record);
    }
    bytes.extend((body.len() as u32).to_be_bytes());
    bytes.extend(body);
    bytes
}
fn knot(selector: u16) -> Vec<u8> {
    let wire: Vec<_> = [0i32, 1 << 24, -1, 1, i32::MIN, i32::MAX]
        .into_iter()
        .flat_map(i32::to_be_bytes)
        .collect();
    record(selector, &wire)
}
fn sample() -> Vec<u8> {
    file(&[
        record(6, &[]),
        record(8, &[0, 1]),
        record(7, &[0; 20]),
        subpath(false, 2),
        knot(1),
        knot(2),
        subpath(true, 2),
        knot(4),
        knot(5),
    ])
}
#[test]
fn retains_unicode_bounds_fill_records_and_exact_signed_coordinates() {
    let library = decode(&sample(), &|| Ok(())).unwrap();
    let shape = &library.shapes()[0];
    assert_eq!(shape.name, "形 🚀");
    assert_eq!(shape.id, "abc");
    assert_eq!(shape.bounds, [10, 20, 110, 220]);
    assert_eq!(shape.records.len(), 9);
    assert_eq!(shape.records[0], Record::FillRule(0));
    assert_eq!(shape.records[1], Record::InitialFill(true));
    assert_eq!(shape.records[2], Record::Clipboard([0; 5]));
    assert_eq!(
        shape.records[3],
        Record::Subpath {
            open: false,
            knots: 2,
            operation: Some(Operation::Combine),
            nonzero_fill: true,
        }
    );
    assert_eq!(
        shape.records[4],
        Record::Knot {
            open: false,
            linked: true,
            points: [1 << 24, 0, 1, -1, i32::MAX, i32::MIN],
        }
    );
    assert!(matches!(
        shape.records[8],
        Record::Knot {
            open: true,
            linked: false,
            ..
        }
    ));
    let empty = b"cush\0\0\0\x02\0\0\0\0";
    assert!(decode(empty, &|| Ok(())).unwrap().shapes().is_empty());
}
#[test]
fn every_truncation_and_trailing_data_is_refused() {
    let bytes = sample();
    for n in 0..bytes.len() {
        assert!(decode(&bytes[..n], &|| Ok(())).is_err(), "prefix {n}");
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode(&trailing, &|| Ok(())).is_err());
}
#[test]
fn multiple_shapes_use_absolute_file_alignment_and_keep_duplicate_ids() {
    let one = sample();
    let mut bytes = one.clone();
    bytes[8..12].copy_from_slice(&2u32.to_be_bytes());
    bytes.extend(&one[12..26]);
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    bytes.extend(&one[28..]);
    let parsed = decode(&bytes, &|| Ok(())).unwrap();
    assert_eq!(parsed.shapes().len(), 2);
    assert_eq!(parsed.shapes()[0], parsed.shapes()[1]);
}
#[test]
fn malformed_topology_unknown_selectors_and_discarded_data_are_refused() {
    for records in [
        vec![knot(1)],
        vec![subpath(false, 1)],
        vec![subpath(false, 1), knot(4)],
        vec![subpath(false, 0), knot(1)],
        vec![subpath(true, 1), record(6, &[]), knot(4)],
        vec![record(99, &[])],
        vec![record(6, &[]), record(6, &[])],
        vec![record(8, &[0, 2])],
        vec![record(7, &[0; 24]), record(7, &[0; 24])],
        vec![record(6, &[0, 0, 1])],
        vec![record(0, &[0, 0, 0, 4])],
        vec![record(0, &[0, 0, 0, 1, 0, 3])],
    ] {
        assert!(decode(&file(&records), &|| Ok(())).is_err(), "{records:?}");
    }
    let mut partial = file(&[]);
    // The independently authored body begins at byte 36, with a 20-byte header.
    partial[32..36].copy_from_slice(&21u32.to_be_bytes());
    partial.push(0);
    assert!(decode(&partial, &|| Ok(())).is_err());
}
#[test]
fn strict_names_and_inverted_bounds_are_refused() {
    for (range, replacement) in [
        (16..18, vec![0xd8, 0]),               // unpaired UTF-16 surrogate
        (16..18, vec![0, 0]),                  // interior null
        (26..28, vec![1, 0]),                  // nonzero alignment padding
        (37..38, vec![0xff]),                  // non-ASCII ID
        (48..52, 0u32.to_be_bytes().to_vec()), // bottom before top
        (52..56, 0u32.to_be_bytes().to_vec()), // right before left
    ] {
        let mut bytes = file(&[]);
        bytes[range].copy_from_slice(&replacement);
        assert!(decode(&bytes, &|| Ok(())).is_err());
    }
}
#[test]
fn resource_allowances_are_checked_before_backend_allocation() {
    let mut bytes = file(&[]);
    bytes[8..12].copy_from_slice(&((SHAPES_MAX + 1) as u32).to_be_bytes());
    assert_eq!(
        decode(&bytes, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    bytes = file(&[]);
    bytes[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        decode(&bytes, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let records = vec![subpath(false, 0); RECORDS_MAX + 1];
    assert_eq!(
        decode(&file(&records), &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        decode(&vec![0; BYTES_MAX + 1], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn cancellation_is_polled_during_records_and_before_backend() {
    let bytes = sample();
    for stop in 0..14 {
        let calls = Cell::new(0);
        let result = decode(&bytes, &|| {
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
