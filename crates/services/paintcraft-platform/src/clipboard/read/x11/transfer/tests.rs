use super::*;
const IMAGE: u32 = 100;
const INCR: u32 = 101;
fn property(type_: u32, format: u8, value: Vec<u8>, after: u32) -> GetPropertyReply {
    GetPropertyReply {
        format,
        sequence: 0,
        length: (value.len().div_ceil(4)) as u32,
        type_,
        bytes_after: after,
        value_len: (value.len() / (usize::from(format) / 8)) as u32,
        value,
    }
}
fn header(bound: u32, maximum: usize) -> Transfer {
    Transfer::new(
        property(INCR, 32, bound.to_ne_bytes().into(), 0),
        INCR,
        IMAGE,
        8,
        maximum,
    )
    .unwrap()
}
fn fetch(transfer: &mut Transfer, offset: u32, wait: bool) {
    assert!(
        matches!(transfer.next(), Step::Fetch { offset: actual, wait: waiting }
        if actual == offset && waiting == wait)
    );
}
fn bytes(transfer: &mut Transfer, expected: &[u8]) {
    let Step::Bytes(actual) = transfer.next() else {
        panic!("missing data")
    };
    assert_eq!(actual, expected);
}
#[test]
fn direct_properties_use_word_offsets_and_preserve_an_unaligned_final_tail() {
    let head = vec![7; CHUNK];
    let mut t = Transfer::new(
        property(IMAGE, 8, head.clone(), 5),
        INCR,
        IMAGE,
        8,
        CHUNK + 5,
    )
    .unwrap();
    bytes(&mut t, &head);
    fetch(&mut t, (CHUNK / 4) as u32, false);
    t.accept(property(IMAGE, 8, vec![1, 2, 3, 4, 5], 0))
        .unwrap();
    bytes(&mut t, &[1, 2, 3, 4, 5]);
    assert!(matches!(t.next(), Step::Done));
}
#[test]
fn incr_deletes_only_complete_properties_and_requires_a_typed_empty_terminator() {
    let mut t = header(8, CHUNK + 7);
    fetch(&mut t, 0, true);
    let head = vec![19; CHUNK];
    t.accept(property(IMAGE, 8, head.clone(), 3)).unwrap();
    bytes(&mut t, &head);
    fetch(&mut t, (CHUNK / 4) as u32, false);
    t.accept(property(IMAGE, 8, vec![1, 2, 3], 0)).unwrap();
    bytes(&mut t, &[1, 2, 3]);
    fetch(&mut t, 0, true);
    t.accept(property(IMAGE, 8, vec![4; 4], 0)).unwrap();
    bytes(&mut t, &[4; 4]);
    fetch(&mut t, 0, true);
    t.accept(property(IMAGE, 8, vec![], 0)).unwrap();
    assert!(matches!(t.next(), Step::Done));
    assert!(t.accept(property(IMAGE, 8, vec![0], 0)).is_err());
}
#[test]
fn metadata_can_also_arrive_incrementally_without_exceeding_256_targets() {
    let bound = 1024_u32.to_ne_bytes().to_vec();
    let mut t = Transfer::new(property(INCR, 32, bound, 0), INCR, 4, 32, 1024).unwrap();
    for group in 0..2 {
        fetch(&mut t, 0, true);
        let list: Vec<_> = (group * 128_u32..(group + 1) * 128)
            .flat_map(u32::to_ne_bytes)
            .collect();
        t.accept(property(4, 32, list.clone(), 0)).unwrap();
        bytes(&mut t, &list);
    }
    fetch(&mut t, 0, true);
    t.accept(property(4, 32, vec![], 0)).unwrap();
    assert!(matches!(t.next(), Step::Done));
    let mut over = Transfer::new(
        property(INCR, 32, 0_u32.to_ne_bytes().into(), 0),
        INCR,
        4,
        32,
        1024,
    )
    .unwrap();
    over.accept(property(4, 32, vec![0; 1028], 0)).unwrap_err();
}
#[test]
fn rejects_spoofed_headers_types_lengths_partial_mutation_and_budget_overrun() {
    for first in [
        property(INCR, 8, vec![0; 4], 0),
        property(INCR, 32, vec![0; 8], 0),
        property(INCR, 32, vec![0; 4], 4),
        property(INCR, 32, 65_u32.to_ne_bytes().into(), 0),
    ] {
        assert!(Transfer::new(first, INCR, IMAGE, 8, 64).is_err());
    }
    for malformed in [
        property(IMAGE + 2, 8, vec![0; 4], 0),
        property(IMAGE, 32, vec![0; 4], 0),
        property(IMAGE, 8, vec![0; CHUNK + 1], 0),
        property(IMAGE, 8, vec![0; 65], 0),
        property(IMAGE, 8, vec![0; 3], 1),
        property(IMAGE, 8, vec![], 1),
        GetPropertyReply {
            value_len: 99,
            ..property(IMAGE, 8, vec![0; 4], 0)
        },
    ] {
        assert!(header(0, 64).accept(malformed).is_err());
    }
    let mut short = header(8, 64);
    assert!(short.accept(property(IMAGE, 8, vec![], 0)).is_err());
    let mut changed = header(0, 64);
    changed.accept(property(IMAGE, 8, vec![0; 4], 4)).unwrap();
    bytes(&mut changed, &[0; 4]);
    assert!(changed.accept(property(IMAGE, 8, vec![0; 3], 0)).is_err());
    let mut over = header(0, 8);
    over.accept(property(IMAGE, 8, vec![0; 8], 0)).unwrap();
    bytes(&mut over, &[0; 8]);
    assert!(over.accept(property(IMAGE, 8, vec![0], 0)).is_err());
}
