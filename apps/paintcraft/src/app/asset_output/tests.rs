//! Actual admitted jobs and retention escapes; no OS picker/provider is invoked.
use super::*;
#[test]
fn cumulative_entries_and_encoded_capacity_are_priced_before_any_job() {
    let doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let one = quote::<paintcraft_formats::icons::Prepared>(&doc, [(128, 128)], 64 << 20).unwrap();
    let max = quote::<paintcraft_formats::icons::Prepared>(
        &doc,
        std::iter::repeat_n((128, 128), 256),
        64 << 20,
    )
    .unwrap();
    assert_eq!(max - one, 255 * 128 * 128 * 24);
    assert!(
        quote::<paintcraft_formats::icons::Prepared>(
            &doc,
            std::iter::repeat_n((256, 256), 256),
            64 << 20
        )
        .is_err()
    );
    assert!(max > (64 << 20) + (64 << 20) * 4);
}
#[test]
fn escaped_prepared_clone_keeps_payload_admitted_after_job_cancel_and_source_drop() {
    let budget = paintcraft_runtime::SharedBudget::new(1 << 20);
    let source = Arc::new(
        paintcraft_document::new_document(
            2,
            1,
            Some(Rgba([64, 16, 128, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap(),
    );
    let lease = budget.reserve(4096).unwrap();
    let owner = Owner {
        source,
        _lease: lease,
    };
    let prepared = Prepared {
        value: Shared::new(vec![3u8; 1024]),
        owner: Some(Shared::new(owner)),
    };
    let escaped = prepared.clone();
    let last = escaped.clone();
    drop(prepared);
    drop(escaped);
    assert_eq!(budget.usage().reserved, 4096);
    assert_eq!(last.len(), 1024);
    drop(last);
    assert_eq!(budget.usage().reserved, 0);
}
