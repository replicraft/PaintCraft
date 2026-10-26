use super::*;
use paintcraft_core::types::json;
use std::cell::Cell;
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/radiance-fixtures/");
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{ROOT}{name}")).unwrap()
}
#[test]
fn admitted_owned_packets_match_independent_lower_edge_samples_without_mutation() {
    let manifest = json::parse(&fixture("manifest.json")).unwrap();
    for entry in manifest.as_array().unwrap() {
        if entry.get("expected_error").unwrap().as_str().is_some() {
            continue;
        }
        let name = entry.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let before = bytes.clone();
        let admitted = inspect(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(1 << 20);
        let decoded = decode(&admitted, budget.clone(), &|| Ok(())).unwrap();
        let golden = fixture(&name.replace(".hdr", ".lower-edge.f32le"));
        let expected: Vec<_> = golden
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v).to_bits())
            .collect();
        assert_eq!(
            decoded
                .samples()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected,
            "{name}"
        );
        assert_eq!(decoded.source_sha256(), admitted.sha256());
        assert_eq!(bytes, before);
        assert_eq!(budget.usage().reserved, decoded.samples.len() as u64 * 4);
        drop(decoded);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn denial_and_decode_read_cancellation_release_all_private_output() {
    let bytes = fixture("modern-rle-8x2.hdr");
    let admitted = inspect(&bytes, 16, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(1);
    let error = decode(&admitted, budget.clone(), &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::DecodedTiles
    );
    assert_eq!(budget.usage().reserved, 0);
    for cancel_at in [1, 2, 10, 40] {
        let calls = Cell::new(0);
        let budget = SharedBudget::new(1 << 20);
        let result = decode(&admitted, budget.clone(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == cancel_at {
                Err(Error::new(ErrorCode::OperationCancelled, "read callback"))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "callback {cancel_at}"
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
