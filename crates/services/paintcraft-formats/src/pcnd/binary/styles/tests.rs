use super::*;
use std::cell::Cell;
fn tail(count: u16) -> Vec<u8> {
    let mut bytes = count.to_be_bytes().to_vec();
    for n in 1..=count {
        bytes.extend_from_slice(&Uuid::from_u128(u128::from(n)).as_bytes()[..]);
        bytes.extend_from_slice(&[1, 1, 220, 40, 0, 128, 0]);
    }
    bytes
}
#[test]
fn defining_style_vector_and_wire_allowances_exact_short_empty_and256_boundary() {
    for count in [1u16, 256] {
        let bytes = tail(count);
        let exact = bytes
            .len()
            .max(usize::from(count) * size_of::<StyleInstance>());
        let mut input = Input::new(&bytes);
        let stack = read(&mut input, exact, &|| Ok(())).unwrap();
        assert_eq!(stack.len(), usize::from(count));
        assert_eq!(input.remaining(), 0);
        assert!(stack.storage_bytes() <= exact);
        let calls = Cell::new(0);
        assert_eq!(
            read(&mut Input::new(&bytes), exact - 1, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(
            calls.get(),
            0,
            "short allowance must refuse before entry allocation/validation callbacks"
        );
    }
    let calls = Cell::new(0);
    let empty = read(&mut Input::new(&[0, 0]), 2, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.digest(), StyleStack::empty().digest());
    assert_eq!(calls.get(), 0);
    assert_eq!(
        read(
            &mut Input::new(&257u16.to_be_bytes()),
            usize::MAX,
            &|| panic!("oversize callback")
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn retained_style_capacity_is_cumulatively_dominated_by_encoded_wire_contribution() {
    let mut retained = Vec::new();
    let mut stored = 0usize;
    let mut encoded = 0usize;
    for _ in 0..32 {
        for count in [0u16, 1, 7, 256] {
            let bytes = tail(count);
            let stack = read(&mut Input::new(&bytes), bytes.len(), &|| Ok(())).unwrap();
            assert!(stack.storage_bytes() <= usize::from(count) * STYLE_RECORD_BYTES);
            stored += stack.storage_bytes();
            encoded += bytes.len();
            retained.push(stack);
        }
    }
    assert!(stored <= encoded);
    assert!(encoded < DOCUMENT_MAX as usize);
    assert!(
        size_of::<StyleInstance>() <= STYLE_RECORD_BYTES,
        "unsupported layouts must refuse before allocation"
    );
}
