use super::*;
use paintcraft_core::types::{
    Rect, json,
    policy::hex,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{Band, Run};
fn cases() -> paintcraft_core::types::json::Value {
    json::parse(include_bytes!(
        "../../../../../../tests/source8-snapshot-fixtures/oracle.json"
    ))
    .unwrap()
}
fn fixture(name: &str) -> Vec<u8> {
    let all = cases();
    let r = all
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    r.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn limits() -> RequirementsLimits {
    RequirementsLimits {
        layers: 2,
        tiles: 2,
        metadata_bytes: 8 << 20,
        coverage: Limits::default(),
        work: 268_435_456,
    }
}
fn read(verified: &CheckedContainer<'_>, budget: &SharedBudget) -> Result<Document> {
    verified.read_with_admission_checked(
        2,
        131072,
        2,
        crate::pcnd::ReadAdmission {
            metadata: crate::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget,
            },
            coverage: crate::pcnd::CoverageAdmission {
                bytes: 8 << 20,
                budget,
                limits: Limits::default(),
            },
        },
        &|| Ok(()),
    )
}
fn probe(bytes: &[u8], scope: &BudgetScope) -> Result<SourceCoverage8> {
    CheckedContainer::verify(bytes, &|| Ok(()))?.probe_layer_blend_appearance_checked(
        limits(),
        scope,
        &|| Ok(()),
        &|_| Ok(()),
    )
}
#[test]
fn independent_literals_zero_own_and_one_authentication_through_actual_adoption() {
    for row in cases().get("cases").unwrap().as_array().unwrap() {
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let accepted = row.get("accepted").unwrap().as_bool().unwrap();
        let source8 = row.get("source8").unwrap().as_bool().unwrap();
        let budget = SharedBudget::new(32 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        PAYLOAD_HASH_PASSES.with(|v| v.set(0));
        let verified = CheckedContainer::verify_with_source_hash(&bytes, &|| Ok(()));
        if name == "bad-sha" {
            assert!(verified.is_err());
            continue;
        }
        let (verified, source_sha) = verified.unwrap();
        assert_eq!(
            hex(&source_sha),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let decoded = read(&verified, &budget);
        assert_eq!(decoded.is_ok(), accepted, "{name}");
        let assessed = verified.probe_layer_blend_appearance_checked(
            limits(),
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        );
        if !source8 || !accepted {
            if name == "valid-old7-mask" {
                assert!(
                    verified
                        .probe_layer_appearance_checked(limits(), &scope, &|| Ok(()), &|_| Ok(()))
                        .is_ok()
                );
            }
            // Borrowed structural accounting defers tile uniqueness to actual read.
            if name != "duplicate-tile" {
                assert!(assessed.is_err(), "{name}");
            }
            continue;
        }
        let source = assessed.unwrap();
        let doc = decoded.unwrap();
        assert_eq!(
            source.wire_tile_records(),
            row.get("wire_tiles").unwrap().as_i64().unwrap() as u64
        );
        assert_eq!(
            source.inventory().own.stored,
            row.get("own_count").unwrap().as_i64().unwrap() as u32
        );
        assert_eq!(
            hex(&source.layer_blend()),
            row.get("blend_sha256").unwrap().as_str().unwrap()
        );
        assert!(
            verified
                .probe_layer_appearance_checked(limits(), &scope, &|| Ok(()), &|_| Ok(()))
                .is_err()
        );
        let projected = source
            .project_semantics_checked(&doc, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap();
        let expected = source.inventory();
        let adopted = source
            .finish_adoption_checked(&doc, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap();
        assert_eq!(projected.source_inventory(), expected);
        assert_eq!(
            adopted
                .candidate_inventory_checked(
                    &doc,
                    &doc,
                    CandidateProvenance::Unchanged,
                    &scope,
                    &|| Ok(()),
                    &|_| Ok(())
                )
                .unwrap(),
            expected
        );
        assert_eq!(projected.layer_blend(), adopted.layer_blend());
        PAYLOAD_HASH_PASSES.with(|v| assert_eq!(v.get(), 1));
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn same_reserved_scope_and_cumulative_work_cancellation_do_not_mint_headroom() {
    let bytes = fixture("own128");
    let verified = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let global = SharedBudget::new(32 << 20);
    let tiny = global
        .reserve_capacity(CONTROL_BYTES - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let callbacks = Cell::new(0);
    assert!(
        verified
            .probe_layer_blend_appearance_checked(
                limits(),
                &tiny,
                &|| {
                    callbacks.set(callbacks.get() + 1);
                    Ok(())
                },
                &|_| Ok(())
            )
            .is_err()
    );
    assert_eq!(callbacks.get(), 0);
    assert_eq!(tiny.footprint().live, 0);
    let scope = global
        .reserve_capacity(transient_quote(2).unwrap())
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let left = Cell::new(511u64);
    let debit = |n| {
        let next = left.get().checked_sub(n).ok_or_else(resource)?;
        left.set(next);
        Ok(())
    };
    assert!(
        verified
            .probe_layer_blend_appearance_checked(limits(), &scope, &|| Ok(()), &debit)
            .is_err()
    );
    assert_eq!(left.get(), 511);
    assert_eq!(scope.footprint().live, 0);
    let count = Cell::new(0);
    let work = Cell::new(0u64);
    verified
        .probe_layer_blend_appearance_checked(
            limits(),
            &scope,
            &|| {
                count.set(count.get() + 1);
                Ok(())
            },
            &|n| {
                work.set(work.get() + n);
                Ok(())
            },
        )
        .unwrap();
    assert!(count.get() > 8 && work.get() > 0);
    for stop in [1, 2, count.get() / 2, count.get()] {
        let calls = Cell::new(0);
        let e = verified
            .probe_layer_blend_appearance_checked(
                limits(),
                &scope,
                &|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == stop {
                        Err(Error::new(ErrorCode::OperationCancelled, "test"))
                    } else {
                        Ok(())
                    }
                },
                &|_| Ok(()),
            )
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::OperationCancelled);
        assert_eq!(scope.footprint().live, 0);
    }
    let mut low = limits();
    low.work = work.get() - 1;
    assert!(
        verified
            .probe_layer_blend_appearance_checked(low, &scope, &|| Ok(()), &|_| Ok(()))
            .is_err()
    );
    assert_eq!(bytes, fixture("own128"));
}
#[test]
fn canonical_live_projection_is_distinct_from_actual_retained_adoption() {
    let budget = SharedBudget::new(32 << 20);
    let bytes = fixture("own128");
    let verified = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let mut live = read(&verified, &budget).unwrap();
    let runs: Vec<_> = (0..64)
        .map(|x| Run {
            x0: x * 2,
            x1: x * 2 + 1,
            coverage: 128,
        })
        .collect();
    let full = A8Mask::from_bands(
        Rect::new(0, 0, 128, 1).unwrap(),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        &budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let clipped = full
        .clipped_checked(Rect::new(0, 0, 1, 1).unwrap(), u64::MAX, &|| Ok(()))
        .unwrap()
        .unwrap();
    assert!(
        clipped.storage_usage().unwrap().1
            > coverage_decode_quote_checked(clipped.stats().bands, clipped.stats().runs).unwrap()
    );
    let id = live.roots[1];
    live.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(true, clipped));
    let encoded = crate::pcnd::write_compact(&live).unwrap();
    let witness = CheckedContainer::verify(&encoded, &|| Ok(())).unwrap();
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let source = witness
        .probe_layer_blend_appearance_checked(limits(), &scope, &|| Ok(()), &|_| Ok(()))
        .unwrap();
    assert!(
        source
            .project_semantics_checked(&live, &scope, &|| Ok(()), &|_| Ok(()))
            .is_ok()
    );
    assert!(
        source
            .finish_adoption_checked(&live, &scope, &|| Ok(()), &|_| Ok(()))
            .is_err()
    );
    let decoded = read(&witness, &budget).unwrap();
    let source = probe(&encoded, &scope).unwrap();
    assert!(
        source
            .finish_adoption_checked(&decoded, &scope, &|| Ok(()), &|_| Ok(()))
            .is_ok()
    );
}
#[test]
fn candidate_duplicate_remove_and_mode_order_fences_keep_source_anchors() {
    let budget = SharedBudget::new(32 << 20);
    let bytes = fixture("own128");
    let witness = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let mut base = read(&witness, &budget).unwrap();
    let id = base.roots[1];
    let mask = base.layers[&id].mask.as_ref().unwrap().coverage().clone();
    let descriptor = paintcraft_core::types::filters::Descriptor::InvertRgbV1;
    let f = FilterInstance::new(Uuid::from_u128(27), false, descriptor)
        .with_mask(Some(paintcraft_document::FilterMask::new(true, mask)));
    base.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![f.clone()]).unwrap();
    let raw = crate::pcnd::write(&base).unwrap();
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let witness = CheckedContainer::verify(&raw, &|| Ok(())).unwrap();
    let base = read(&witness, &budget).unwrap();
    let f = base.layers[&id].filters.iter().next().unwrap().clone();
    let token = probe(&raw, &scope)
        .unwrap()
        .finish_adoption_checked(&base, &scope, &|| Ok(()), &|_| Ok(()))
        .unwrap();
    let before = token.source_inventory();
    assert_eq!(before.occurrences, 2);
    let new = Uuid::from_u128(28);
    let mut duplicate = base.clone();
    duplicate.layers.get_mut(&id).unwrap().filters =
        FilterStack::new(vec![f.clone(), f.with_id(new)]).unwrap();
    let provenance = CandidateProvenance::Duplicate {
        layer: id,
        source_filter: Uuid::from_u128(27),
        new_filter: new,
    };
    let after = token
        .candidate_inventory_checked(&base, &duplicate, provenance, &scope, &|| Ok(()), &|_| {
            Ok(())
        })
        .unwrap();
    assert_eq!(after.occurrences, 3);
    assert_eq!(after.own, before.own);
    assert_eq!(after.source_coverage, before.source_coverage);
    assert!(
        token
            .candidate_inventory_checked(
                &base,
                &duplicate,
                CandidateProvenance::Unchanged,
                &scope,
                &|| Ok(()),
                &|_| Ok(())
            )
            .is_err()
    );
    let mut removed = base.clone();
    removed.layers.get_mut(&id).unwrap().filters = FilterStack::default();
    assert_eq!(
        token
            .candidate_inventory_checked(
                &base,
                &removed,
                CandidateProvenance::Unchanged,
                &scope,
                &|| Ok(()),
                &|_| Ok(())
            )
            .unwrap()
            .occurrences,
        1
    );
    let mut wrong = base.clone();
    wrong.roots.swap(0, 1);
    assert!(
        token
            .candidate_inventory_checked(
                &base,
                &wrong,
                CandidateProvenance::Unchanged,
                &scope,
                &|| Ok(()),
                &|_| Ok(())
            )
            .is_err()
    );
    wrong = base.clone();
    wrong.layers.get_mut(&id).unwrap().blend_mode = paintcraft_document::LayerBlendMode::Screen;
    assert!(
        token
            .candidate_inventory_checked(
                &base,
                &wrong,
                CandidateProvenance::Unchanged,
                &scope,
                &|| Ok(()),
                &|_| Ok(())
            )
            .is_err()
    );
}
