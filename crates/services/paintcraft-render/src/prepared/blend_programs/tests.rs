use super::*;
use paintcraft_core::types::{
    Rgba,
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
fn doc() -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = LayerBlendMode::Screen;
    doc
}
fn filters(descriptors: Vec<Descriptor>) -> FilterStack {
    FilterStack::new(
        descriptors
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                FilterInstance::new(deterministic_id(&["capture-key", &i.to_string()]), true, d)
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn mixed_canonical_semantic_keys_and_constructor_ledger_are_complete() {
    let mut doc = doc();
    let descriptors = vec![
        Descriptor::CurvesRgbV1(CurvesRgbV1::new([255; 5])),
        Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(-1).unwrap()),
        Descriptor::CurvesRgbV1(CurvesRgbV1::new([0; 5])),
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(0).unwrap()),
        Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
    ];
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = filters(descriptors);
    let b = SharedBudget::new(4 << 20);
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, u64::MAX);
    let keys = Keys::capture(&doc, &a, None, false, &|| Ok(()), &|n| a.reserve_work(n)).unwrap();
    assert_eq!(keys.lengths, [1, 2, 2]);
    assert_eq!(
        keys.exposures[..2]
            .iter()
            .map(|p| p.semantic_bytes())
            .collect::<Vec<_>>(),
        [[0, 0], [255, 255]]
    );
    assert_eq!(
        keys.curves[..2]
            .iter()
            .map(|p| p.semantic_bytes())
            .collect::<Vec<_>>(),
        [[0; 5], [255; 5]]
    );
    let ql = family_quote(1, 6, 1088).unwrap();
    let qe = family_quote(2, 2, 576).unwrap();
    let qc = family_quote(2, 5, 64).unwrap();
    assert_eq!(keys.quote_metadata().unwrap(), ql + qe + qc);
    let duplicates = ledger::duplicates(1, 6).unwrap()
        + ledger::duplicates(2, 2).unwrap()
        + ledger::duplicates(2, 5).unwrap();
    assert_eq!(
        keys.constructor_metadata().unwrap(),
        3 * ql + 2 * qe + qc + duplicates + 2176 + 2 * 1664 + 2 * 192
    );
    assert_eq!(a.work_used(), keys.capture_work);
    assert!(!a.has_prepared_programs());
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn duplicate_at_full_combined_limit_succeeds_and_new_257th_key_refuses() {
    let mut doc = doc();
    let full: Vec<_> = (0..=255)
        .map(|i| Descriptor::CurvesRgbV1(CurvesRgbV1::new([i, 0, 0, 0, 0])))
        .collect();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = filters(full);
    let mut extra = doc.layers[&doc.roots[0]].clone();
    extra.id = deterministic_id(&["extra-root"]);
    extra.filters = filters(vec![Descriptor::CurvesRgbV1(CurvesRgbV1::new([0; 5]))]);
    let id = extra.id;
    doc.roots.push(id);
    doc.layers.insert(id, extra);
    let b = SharedBudget::new(4 << 20);
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, u64::MAX);
    let capture = || Keys::capture(&doc, &a, None, false, &|| Ok(()), &|n| a.reserve_work(n));
    assert_eq!(capture().unwrap().lengths, [0, 0, 256]);
    doc.layers.get_mut(&id).unwrap().filters =
        filters(vec![Descriptor::ExposureSrgbV1(ExposureSrgbV1::default())]);
    assert_eq!(
        Keys::capture(&doc, &a, None, false, &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(b.usage().reserved, 0);
}
