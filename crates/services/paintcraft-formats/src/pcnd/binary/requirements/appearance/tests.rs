use super::*;
use paintcraft_core::types::{
    Rect,
    json::{self},
    resources::AllocationClass,
};
use paintcraft_document::{FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{Band, Run};
fn limits() -> RequirementsLimits {
    RequirementsLimits {
        layers: 2,
        tiles: 2,
        metadata_bytes: 8 << 20,
        coverage: Limits::default(),
        work: 268_435_456,
    }
}
fn fixture(name: &str) -> Vec<u8> {
    let value = json::parse(include_bytes!(
        "../../../../../tests/pcnd-layer-mask-fixtures/oracle.json"
    ))
    .unwrap();
    let row = value
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    row.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn adopt(wire: &[u8], budget: &SharedBudget) -> Document {
    crate::pcnd::read_with_admission_checked(
        wire,
        2,
        131_072,
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
    .unwrap()
}
#[test]
fn independent_pcnd7_masks_probe_then_actual_adoption_and_legacy_resolver_stays_closed() {
    for name in ["bands", "disabled", "zero", "mixed", "two-layers"] {
        let wire = fixture(name);
        let budget = SharedBudget::new(32 << 20);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let calls = Cell::new(0);
        let work = Cell::new(0u64);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let spend = |n| {
            work.set(work.get() + n);
            Ok(())
        };
        let verified = CheckedContainer::verify(&wire, &check).unwrap();
        assert!(
            verified
                .resolve_requirements_checked(limits(), &scope, &check, |_, _| Ok(()))
                .is_err()
        );
        let source = verified
            .probe_layer_appearance_checked(limits(), &scope, &check, &spend)
            .unwrap();
        let facts = source.inventory();
        assert!(facts.own.stored > 0);
        assert_eq!(
            source.wire_tile_records(),
            verified.snapshot_tile_records_checked(&check).unwrap()
        );
        let doc = adopt(&wire, &budget);
        let projected = source
            .project_semantics_checked(&doc, &scope, &check, &spend)
            .unwrap();
        assert_eq!(projected.source_inventory(), facts);
        let adopted = source
            .finish_adoption_checked(&doc, &scope, &check, &spend)
            .unwrap();
        assert_eq!(
            adopted
                .candidate_inventory_checked(
                    &doc,
                    &doc,
                    CandidateProvenance::Unchanged,
                    &scope,
                    &check,
                    &spend
                )
                .unwrap(),
            facts
        );
        assert!(calls.get() > 0 && work.get() > 0);
    }
}
#[test]
fn candidate_duplicate_remove_and_instance_toggle_change_only_actual_per_use_inventory() {
    let wire = fixture("mixed");
    let budget = SharedBudget::new(32 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let source = CheckedContainer::verify(&wire, &|| Ok(()))
        .unwrap()
        .probe_layer_appearance_checked(limits(), &scope, &|| Ok(()), &|_| Ok(()))
        .unwrap();
    let doc = adopt(&wire, &budget);
    let before = source.inventory();
    let adopted = source
        .finish_adoption_checked(&doc, &scope, &|| Ok(()), &|_| Ok(()))
        .unwrap();
    let layer = *doc.roots.first().unwrap();
    let first = doc.layers[&layer].filters.iter().next().unwrap().clone();
    assert!(first.mask().is_some());
    let mut toggle = doc.clone();
    toggle.layers.get_mut(&layer).unwrap().filters =
        FilterStack::new(vec![first.with_enabled(!first.enabled())]).unwrap();
    let result = adopted
        .candidate_inventory_checked(
            &doc,
            &toggle,
            CandidateProvenance::Unchanged,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap();
    assert_eq!(result.own, before.own);
    assert_eq!(result.source_coverage, before.source_coverage);
    assert_ne!(result.mask_inventory, before.mask_inventory);
    preflight_duplicate_counts(&doc, layer, first.id(), &|| Ok(()), &|_| Ok(())).unwrap();
    let new = Uuid::from_bytes([0xab; 16]);
    let mut duplicate = doc.clone();
    duplicate.layers.get_mut(&layer).unwrap().filters =
        FilterStack::new(vec![first.clone(), first.with_id(new)]).unwrap();
    let provenance = CandidateProvenance::Duplicate {
        layer,
        source_filter: first.id(),
        new_filter: new,
    };
    let result =
        adopted
            .candidate_inventory_checked(&doc, &duplicate, provenance, &scope, &|| Ok(()), &|_| {
                Ok(())
            })
            .unwrap();
    assert_eq!(result.occurrences, before.occurrences + 1);
    assert_eq!(result.own, before.own);
    assert_eq!(result.source_coverage, before.source_coverage);
    assert!(
        adopted
            .candidate_inventory_checked(
                &doc,
                &duplicate,
                CandidateProvenance::Unchanged,
                &scope,
                &|| Ok(()),
                &|_| Ok(())
            )
            .is_err()
    );
    let mut removed = doc.clone();
    removed.layers.get_mut(&layer).unwrap().filters = FilterStack::default();
    let result = adopted
        .candidate_inventory_checked(
            &doc,
            &removed,
            CandidateProvenance::Unchanged,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap();
    assert_eq!(result.occurrences, before.occurrences - 1);
    assert_eq!(result.own, before.own);
}
#[test]
fn caller_control_preflight_and_cumulative_work_refuse_without_source_mutation() {
    let wire = fixture("bands");
    let verified = CheckedContainer::verify(&wire, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(0);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let calls = Cell::new(0);
    assert!(
        verified
            .probe_layer_appearance_checked(
                limits(),
                &scope,
                &|| {
                    calls.set(calls.get() + 1);
                    Ok(())
                },
                &|_| Ok(())
            )
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    let budget = SharedBudget::new(32 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let left = Cell::new(STEP_WORK - 1);
    let denied = |n| {
        left.set(left.get().checked_sub(n).ok_or_else(resource)?);
        Ok(())
    };
    assert!(
        verified
            .probe_layer_appearance_checked(limits(), &scope, &|| Ok(()), &denied)
            .is_err()
    );
    assert_eq!(wire, fixture("bands"));
    assert_eq!(left.get(), STEP_WORK - 1);
    assert!(coverage_decode_quote_checked(262_145, 0).is_err());
    // A fresh lookup itself must be denied before the bounded filter find,
    // independently of earlier512-slot traversal/capture checks.
    let mixed = fixture("mixed");
    let doc = adopt(&mixed, &budget);
    let layer = doc.roots[0];
    let filter = doc.layers[&layer].filters.iter().next().unwrap().id();
    let lookup = Cell::new(false);
    let denied_lookup = |n| {
        if n == FILTER_LOOKUP_WORK {
            lookup.set(true);
            Err(resource())
        } else {
            Ok(())
        }
    };
    assert_eq!(
        preflight_duplicate_counts(&doc, layer, filter, &|| Ok(()), &denied_lookup)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(lookup.get());
}
#[test]
fn main_live_clipped_storage_projection_is_distinct_from_worker_adoption_attestation() {
    let wire = fixture("bands");
    let budget = SharedBudget::new(32 << 20);
    let mut live = adopt(&wire, &budget);
    let runs: Vec<_> = (0..64)
        .map(|x| Run {
            x0: x * 2,
            x1: x * 2 + 1,
            coverage: 127,
        })
        .collect();
    let bands = [Band {
        y0: 0,
        y1: 1,
        runs: &runs,
    }];
    let full = A8Mask::from_bands(
        Rect::new(0, 0, 128, 1).unwrap(),
        &bands,
        &budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let view = full
        .clipped_checked(Rect::new(0, 0, 1, 1).unwrap(), u64::MAX, &|| Ok(()))
        .unwrap()
        .unwrap();
    assert!(
        view.storage_usage().unwrap().1
            > coverage_decode_quote_checked(view.stats().bands, view.stats().runs).unwrap()
    );
    let id = live.roots[0];
    live.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(true, view));
    let encoded = crate::pcnd::write_compact(&live).unwrap();
    let scope = budget.scope(AllocationClass::SharedBudget);
    let verified = CheckedContainer::verify(&encoded, &|| Ok(())).unwrap();
    let source = verified
        .probe_layer_appearance_checked(limits(), &scope, &|| Ok(()), &|_| Ok(()))
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
    let decoded = adopt(&encoded, &budget);
    let source = verified
        .probe_layer_appearance_checked(limits(), &scope, &|| Ok(()), &|_| Ok(()))
        .unwrap();
    assert!(
        source
            .finish_adoption_checked(&decoded, &scope, &|| Ok(()), &|_| Ok(()))
            .is_ok()
    );
}
