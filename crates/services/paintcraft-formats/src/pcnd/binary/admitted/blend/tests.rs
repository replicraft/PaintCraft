use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::LayerBlendMode;
use std::cell::Cell;
fn fixture(name: &str) -> Vec<u8> {
    let x = json::parse(include_bytes!(
        "../../../../../tests/source8-snapshot-fixtures/oracle.json"
    ))
    .unwrap();
    let row = x
        .get("cases")
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
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn document(name: &str) -> Document {
    let wire = fixture(name);
    let budget = SharedBudget::new(16 << 20);
    crate::pcnd::read_with_admission_checked(
        &wire,
        2,
        131072,
        2,
        crate::pcnd::ReadAdmission {
            metadata: crate::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: crate::pcnd::CoverageAdmission {
                bytes: 8 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
#[derive(Debug)]
struct Encoded {
    bytes: Vec<u8>,
    _account: paintcraft_core::types::resources::BudgetAccount,
}
fn encode(
    doc: &Document,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Encoded> {
    // Output is an independent caller declaration, simultaneous with scratch.
    let mut output = scope.account();
    output.resize(65536)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(65536).map_err(|_| limit())?;
    output.resize(bytes.capacity() as u64)?;
    let result =
        write_portable_blend_snapshot_admitted_checked(doc, &mut bytes, scope, check, spend);
    if let Err(e) = result {
        drop(bytes);
        return Err(e);
    }
    // Consumer owns the same account; field order frees Vec before its charge.
    Ok(Encoded {
        bytes,
        _account: output,
    })
}
#[test]
fn independent_no_tile_bytes_and_qualified_portable_choice_match_without_old7_widening() {
    for name in [
        "zero-own-multiply",
        "zero-own-screen",
        "hidden-multiply",
        "opacity0-screen",
        "own128",
        "disabled-own128",
        "enabled-own-zero",
        "proof-filter-disabled",
        "explicit-default-record",
        "explicit-nondefault-record",
    ] {
        let doc = document(name);
        let frozen = crate::pcnd::write(&doc).unwrap();
        let quote = portable_blend_snapshot_scratch_quote(&doc).unwrap() + 65536;
        let realm = SharedBudget::new(quote);
        let scope = realm
            .reserve_capacity(quote)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let encoded = encode(&doc, &scope, &|| Ok(()), &|_| Ok(())).unwrap();
        let actual = &encoded.bytes;
        let mut legacy = Vec::new();
        super::super::super::write_encoded(
            &doc,
            Output::new(&mut legacy),
            &|| Ok(()),
            compression::Encoder::portable(),
        )
        .unwrap();
        assert_eq!(actual, &legacy, "{name}");
        if !name.starts_with("explicit-") {
            assert_eq!(actual, &fixture(name), "{name}");
        }
        assert_eq!(actual[4], 8);
        assert_eq!(scope.footprint().live, actual.capacity() as u64);
        drop(encoded);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(crate::pcnd::write(&doc).unwrap(), frozen);
        let mut rejected = Vec::new();
        assert_eq!(
            write_portable_snapshot_admitted_checked(
                &doc,
                &mut rejected,
                &scope,
                &|| Ok(()),
                &|_| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(rejected.is_empty());
    }
}
#[test]
fn canonical_default_elision_and_disabled_identity_filter_change_proof_not_mode_identity() {
    let live = document("explicit-default-record");
    let id = live.roots[0];
    assert_eq!(live.layers[&id].raster.resident_tiles(), 1);
    let q = portable_blend_snapshot_scratch_quote(&live).unwrap() + 65536;
    let budget = SharedBudget::new(q);
    let scope = budget
        .reserve_capacity(q)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let encoded = encode(&live, &scope, &|| Ok(()), &|_| Ok(())).unwrap();
    let actual = &encoded.bytes;
    assert_eq!(actual, &fixture("zero-own-multiply"));
    let witness = crate::pcnd::CheckedContainer::verify(actual, &|| Ok(())).unwrap();
    assert_eq!(
        witness
            .snapshot_tile_records_checked(&|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let coverage = witness
        .probe_layer_blend_appearance_checked(
            crate::pcnd::RequirementsLimits {
                layers: 2,
                tiles: 2,
                metadata_bytes: 8 << 20,
                coverage: Default::default(),
                work: 268_435_456,
            },
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap();
    assert_eq!(coverage.wire_tile_records(), 0);
    let canonical = document("zero-own-multiply");
    assert!(live.same_semantics(&canonical));
    assert_eq!(canonical.layers[&id].raster.resident_tiles(), 0);
    let canvas = live.canvas_rect();
    let a = paintcraft_render::blend::quote_document(&live, canvas).unwrap();
    let b = paintcraft_render::blend::quote_document(&canonical, canvas).unwrap();
    assert_eq!((a.general_samples, a.opaque_samples), (2, 0));
    assert_eq!((b.general_samples, b.opaque_samples), (0, 2));
    let filtered = document("proof-filter-disabled");
    let c = paintcraft_render::blend::quote_document(&filtered, canvas).unwrap();
    assert_eq!((c.general_samples, c.opaque_samples), (2, 0));
    let sha =
        crate::pcnd::layer_blend_identity_checked(&canonical, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap();
    assert_eq!(
        sha,
        crate::pcnd::layer_blend_identity_checked(&filtered, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap()
    );
}
#[test]
fn injected_scope_peak_minus_one_and_cancel_work_refusal_preserve_source() {
    let doc = document("explicit-nondefault-record");
    let before = crate::pcnd::write(&doc).unwrap();
    let quote = portable_blend_snapshot_scratch_quote(&doc).unwrap() + 65536;
    let global = SharedBudget::new(quote * 3);
    let scope = global
        .reserve_capacity(quote)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    encode(&doc, &scope, &|| Ok(()), &|_| Ok(())).unwrap();
    let peak = scope.footprint().peak;
    let small = global
        .reserve_capacity(peak - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    assert_eq!(
        encode(&doc, &small, &|| Ok(()), &|_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(small.footprint().live, 0);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 7 {
            Err(Error::new(ErrorCode::OperationCancelled, "test"))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        encode(&doc, &scope, &check, &|_| Ok(())).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(
        encode(&doc, &scope, &|| Ok(()), &|_| Err(limit()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(crate::pcnd::write(&doc).unwrap(), before);
}
#[test]
fn nonminimal_all_normal_refuses_before_output_and_unknown_root_shape_never_writes() {
    let mut doc = document("zero-own-multiply");
    for layer in doc.layers.values_mut() {
        layer.blend_mode = LayerBlendMode::Normal;
    }
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut output = Vec::new();
    assert!(
        write_portable_blend_snapshot_admitted_checked(
            &doc,
            &mut output,
            &scope,
            &|| Ok(()),
            &|_| Ok(())
        )
        .is_err()
    );
    assert!(output.is_empty());
    assert_eq!(crate::pcnd::write(&doc).unwrap()[4], 1);
    let mut doc = document("zero-own-multiply");
    doc.roots[1] = doc.roots[0];
    assert!(
        write_portable_blend_snapshot_admitted_checked(
            &doc,
            &mut output,
            &scope,
            &|| Ok(()),
            &|_| Ok(())
        )
        .is_err()
    );
    assert!(output.is_empty());
    assert_eq!(scope.footprint().live, 0);
}

#[test]
fn rooted_writer_has_same_wire_and_work_with_large_deleted_layer_table_backing() {
    let compact = document("proof-filter-disabled");
    let mut sparse = document("proof-filter-disabled");
    let template = sparse.layers[&sparse.roots[0]].clone();
    sparse.layers.try_reserve(4096).unwrap();
    let allocated_capacity = sparse.layers.capacity();
    for i in 0..4096 {
        let mut extra = template.clone();
        extra.id = uuid::Uuid::from_u128(10000 + i);
        assert!(sparse.layers.insert(extra.id, extra).is_none());
    }
    for i in 0..4096 {
        assert!(
            sparse
                .layers
                .remove(&uuid::Uuid::from_u128(10000 + i))
                .is_some()
        );
    }
    assert!(allocated_capacity >= 4096);
    assert_eq!(sparse.layers.len(), compact.layers.len());
    assert!(sparse.layers.capacity() > compact.layers.capacity());
    assert!(sparse.same_semantics(&compact));
    assert_eq!(sparse.state_id, compact.state_id);
    let scratch = portable_blend_snapshot_scratch_quote(&compact).unwrap();
    assert_eq!(
        portable_blend_snapshot_scratch_quote(&sparse).unwrap(),
        scratch
    );
    let budget = SharedBudget::new(scratch + 65536);
    let scope = budget
        .reserve_capacity(scratch + 65536)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let expected = fixture("proof-filter-disabled");
    let mut measured = Vec::new();
    for doc in [&compact, &sparse] {
        let work = Cell::new(0u64);
        let encoded = encode(doc, &scope, &|| Ok(()), &|n| {
            work.set(work.get().checked_add(n).ok_or_else(limit)?);
            Ok(())
        })
        .unwrap();
        assert_eq!(encoded.bytes, expected);
        measured.push(work.get());
        assert_eq!(scope.footprint().live, encoded.bytes.capacity() as u64);
        drop(encoded);
        assert_eq!(scope.footprint().live, 0);
    }
    assert_eq!(measured[0], measured[1]);
    assert!(measured[0] >= 5 * 2 * 2048);
    assert!(sparse.same_semantics(&compact));
}

#[test]
fn root_lookup_precharge_refuses_before_validator_allocates_or_sink_writes() {
    let doc = document("zero-own-multiply");
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let callbacks = Cell::new(0);
    let charges = Cell::new(0);
    let mut output = Vec::new();
    let error = write_portable_blend_snapshot_admitted_checked(
        &doc,
        &mut output,
        &scope,
        &|| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        },
        &|units| {
            charges.set(charges.get() + 1);
            assert_eq!(units, doc.roots.len() as u64 * 2048);
            Err(limit())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(charges.get(), 1);
    assert_eq!(callbacks.get(), 1);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(scope.footprint().peak, 0);
    assert!(output.is_empty());
}
