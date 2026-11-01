use super::*;
fn grant(noise: bool) -> Admission7 {
    Admission7 {
        mode: if noise { Mode7::Noise } else { Mode7::Base },
        work_authority: Origin7::CapturedJob,
        workspace_bytes: 64 << 20,
        base_work: 268_435_456,
        noise_work: if noise { 1 << 35 } else { 0 },
    }
}
fn required(noise: bool) -> Requirements7 {
    let mut r = Requirements::default();
    if noise {
        r.insert("add-noise-rgb-v1", "params-v1").unwrap();
    }
    Requirements7::new(r, appearance_registration(), &|| Ok(())).unwrap()
}
fn replace(mut value: Value, name: &str, new: Value) -> Value {
    let Value::Object(ref mut fields) = value else {
        panic!("object")
    };
    fields.insert(name.into(), new);
    value
}
fn pair_value(r: &Requirements7) -> Value {
    Value::Array(
        r.requirements()
            .pairs()
            .map(|(p, c)| {
                Value::object([("profile_id", Value::str(p)), ("codec_id", Value::str(c))])
            })
            .collect(),
    )
}
fn full_fact(r: &Requirements7, a: Admission7) -> Value {
    let own = LayerMasks7 {
        stored: 1,
        enabled: 1,
        bands: 0,
        runs: 0,
        decode_upper_bytes: 0,
        identity: [0x11; 32],
        abi: CoverageAbi::current().unwrap(),
    };
    let inventory = metadata::Inventory7 {
        own,
        occurrences: 1,
        bands: 0,
        runs: 0,
        decode_upper_bytes: 0,
        source_coverage: [0x22; 32],
        mask_inventory: [0x33; 32],
    };
    let fact = Fact7::capture(r, a, inventory, &|| Ok(())).unwrap();
    replace(fact.value(), "pairs", pair_value(r))
}
fn owner() -> Value {
    Value::object([
        (
            "session",
            Value::str("11111111-1111-1111-1111-111111111111"),
        ),
        ("state", Value::str("22222222-2222-2222-2222-222222222222")),
        ("epoch", Value::str("1")),
        ("frontend", Value::Int(3)),
    ])
}
fn control(noise: bool) -> Value {
    let a = grant(noise);
    let r = required(noise);
    Value::object([
        ("owner", owner()),
        ("draft", Value::str("33333333-3333-3333-3333-333333333333")),
        (
            "document",
            Value::str("44444444-4444-4444-4444-444444444444"),
        ),
        ("sourceSha256", Value::str("00".repeat(32))),
        ("size", Value::Array(vec![Value::Int(2), Value::Int(1)])),
        ("layers", Value::Int(1)),
        ("residentBytes", Value::Int(0)),
        ("metadataBytes", Value::Int(0)),
        ("metadataCharge", Value::Int(0)),
        ("filterDigest", Value::str("55".repeat(32))),
        ("maskCount", Value::Int(0)),
        ("maskBands", Value::Int(0)),
        ("maskRuns", Value::Int(0)),
        ("maskDecodeBytes", Value::Int(0)),
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("sourceRequirements", full_fact(&r, a)),
        ("appearance", appearance_registration().value()),
        ("admission", a.value().unwrap()),
        ("slice", a.slice_value()),
    ])
}
#[test]
fn filterless_own_mask_open_adds_only_own_decode_and_new_controls_without_changing_base14() {
    let (base, p) = open(&text(&control(false)), &|| Ok(())).unwrap();
    assert_eq!(base.masks.masks, 0);
    assert_eq!(p.aggregate_coverage(&base).unwrap().masks, 1);
    assert_eq!(
        p.source_quote(&base).unwrap(),
        base.quote
            + p.admission.workspace_bytes
            + CONTROL_BYTES
            + paintcraft_formats::pcnd::layer_appearance::transient_quote(base.layers).unwrap()
            - paintcraft_formats::pcnd::layer_appearance::CONTROL_BYTES
    );
    let mut altered = control(false);
    altered = replace(altered, "registryContract", Value::Int(2));
    assert!(open(&text(&altered), &|| Ok(())).is_err());
    assert!(
        open(
            &text(&replace(control(false), "extra", Value::Null)),
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn hello_requests_subset_only_and_base_cannot_admit_stored_noise_or_change_compiled_slice() {
    let make = |noise| {
        let a = grant(noise);
        Value::object([
            ("protocol", Value::Int(7)),
            ("registryContract", Value::Int(3)),
            ("pairs", pair_value(&required(noise))),
            ("appearance", appearance_registration().value()),
            ("admission", a.value().unwrap()),
            ("slice", a.slice_value()),
        ])
    };
    let empty = hello(&text(&make(false)), CONTROL_BYTES, &|| Ok(())).unwrap();
    assert!(
        field(&empty, "entries")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(empty.get("capabilitySha256").is_none());
    let mut denied = make(false);
    denied = replace(denied, "pairs", pair_value(&required(true)));
    assert!(hello(&text(&denied), CONTROL_BYTES, &|| Ok(())).is_err());
    assert!(hello(&text(&make(true)), CONTROL_BYTES, &|| Ok(())).is_ok());
    let wrong = replace(
        make(false),
        "slice",
        replace(grant(false).slice_value(), "nativeEdge", Value::Int(32)),
    );
    assert!(hello(&text(&wrong), CONTROL_BYTES, &|| Ok(())).is_err());
    assert!(hello(&text(&make(false)), CONTROL_BYTES - 1, &|| Ok(())).is_err());
}
#[test]
fn source_fact_hash_and_immutable_anchor_mismatch_refuse_without_counter_or_adoption() {
    let value = control(false);
    let fact = field(&value, "sourceRequirements").unwrap().clone();
    let value = replace(
        value,
        "sourceRequirements",
        replace(fact, "capabilitySha256", Value::str("00".repeat(32))),
    );
    assert!(open(&text(&value), &|| Ok(())).is_err());
    let malformed = replace(
        control(false),
        "admission",
        replace(
            grant(false).value().unwrap(),
            "baseWorkLimit",
            Value::str("0268435456"),
        ),
    );
    assert!(open(&text(&malformed), &|| Ok(())).is_err());
}

#[test]
fn begin_quote_only_keeps_source_anchor_and_refuses_changed_admission_or_actual_abi() {
    let (source, profile) = open(&text(&control(false)), &|| Ok(())).unwrap();
    let value = begin_value(&source, &profile);
    let parsed = begin(&text(&value), &|| Ok(())).unwrap();
    assert!(!parsed.render);
    parsed.matches_source(&source, &profile).unwrap();
    let wrong = replace(value, "admissionSha256", Value::str("00".repeat(32)));
    assert!(
        begin(&text(&wrong), &|| Ok(()))
            .unwrap()
            .matches_source(&source, &profile)
            .is_err()
    );
    let value = control(false);
    let fact = field(&value, "sourceRequirements").unwrap().clone();
    let own = field(&fact, "layerMasks").unwrap().clone();
    let other = if CoverageAbi::current().unwrap() == CoverageAbi::Wasm32V1 {
        "native64-v1"
    } else {
        "wasm32-v1"
    };
    let fact = replace(
        fact,
        "layerMasks",
        replace(own, "coverageAbi", Value::str(other)),
    );
    let error = open(&text(&replace(value, "sourceRequirements", fact)), &|| {
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}

fn begin_value(source: &Source, profile: &Profile) -> Value {
    let union = Union7::capture(
        &profile.requirements,
        profile.admission,
        profile.source.source_coverage,
        &|| Ok(()),
    )
    .unwrap();
    Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("owner", owner()),
        ("draft", field(&source.value, "draft").unwrap().clone()),
        (
            "sourceSha256",
            field(&source.value, "sourceSha256").unwrap().clone(),
        ),
        ("generation", Value::str("1")),
        ("operation", Value::Null),
        ("target", Value::Null),
        (
            "view",
            Value::object([
                (
                    "region",
                    Value::Array(vec![
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(2),
                        Value::Int(1),
                    ]),
                ),
                ("level", Value::Int(0)),
            ]),
        ),
        ("render", Value::Bool(false)),
        ("candidateRequirements", profile.source.value()),
        ("unionRequirements", union.value()),
        (
            "admissionSha256",
            Value::str(hex(&profile.admission.digest().unwrap())),
        ),
    ])
}

#[test]
fn source7_envelope_uuid_spellings_are_canonical_without_changing_legacy_parser() {
    const CANONICAL: &str = "abcdef01-2345-6789-abcd-ef0123456789";
    fn with_id(value: Value, name: &str, spelling: &str) -> Value {
        if matches!(name, "session" | "state") {
            let owner = field(&value, "owner").unwrap().clone();
            replace(value, "owner", replace(owner, name, Value::str(spelling)))
        } else {
            replace(value, name, Value::str(spelling))
        }
    }
    let (source, profile) = open(&text(&control(false)), &|| Ok(())).unwrap();
    let begin_control = begin_value(&source, &profile);
    let alternatives = [
        CANONICAL.replace('-', ""),
        CANONICAL.to_ascii_uppercase(),
        format!("urn:uuid:{CANONICAL}"),
        format!("{{{CANONICAL}}}"),
    ];
    for name in ["session", "state", "draft", "document"] {
        let valid = with_id(control(false), name, CANONICAL);
        open(&text(&valid), &|| Ok(())).unwrap();
        for spelling in &alternatives {
            // Each alternate is a valid UUID to the old shared parser. This
            // new envelope guard must refuse it before adoption or execution.
            crate::protocol::uuid(&Value::str(spelling)).unwrap();
            assert_eq!(
                open(&text(&with_id(valid.clone(), name, spelling)), &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema,
                "OPEN {name}: {spelling}"
            );
        }
        if name == "document" {
            continue;
        }
        let valid = with_id(begin_control.clone(), name, CANONICAL);
        begin(&text(&valid), &|| Ok(())).unwrap();
        for spelling in &alternatives {
            assert_eq!(
                begin(&text(&with_id(valid.clone(), name, spelling)), &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema,
                "BEGIN {name}: {spelling}"
            );
        }
    }
}

#[test]
fn admission_work_authority_is_required_closed_and_digest_bound() {
    for noise in [false, true] {
        for origin in [Origin7::CapturedJob, Origin7::StandaloneViewJobs] {
            let a = Admission7 {
                work_authority: origin,
                ..grant(noise)
            };
            let positive = replace(
                replace(control(noise), "admission", a.value().unwrap()),
                "sourceRequirements",
                full_fact(&required(noise), a),
            );
            let (_, profile) = open(&text(&positive), &|| Ok(())).unwrap();
            assert_eq!(profile.admission.work_authority, origin);
        }
    }
    let value = control(false);
    let grant = field(&value, "admission").unwrap().clone();
    for invalid in [
        Value::Null,
        Value::Bool(true),
        Value::str(""),
        Value::str("unknown"),
        Value::str("CapturedJob"),
    ] {
        let changed = replace(
            value.clone(),
            "admission",
            replace(grant.clone(), "workAuthority", invalid),
        );
        assert_eq!(
            open(&text(&changed), &|| Ok(())).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    let Value::Object(mut fields) = grant.clone() else {
        panic!("object")
    };
    fields.remove("workAuthority");
    let missing = replace(value.clone(), "admission", Value::Object(fields));
    assert_eq!(
        open(&text(&missing), &|| Ok(())).unwrap_err().code,
        ErrorCode::InputSchema
    );
    let changed = replace(
        value,
        "admission",
        replace(grant, "workAuthority", Value::str("standalone-view-jobs")),
    );
    // Structurally valid origin change cannot reuse a captured-origin Fact SHA.
    assert_eq!(
        open(&text(&changed), &|| Ok(())).unwrap_err().code,
        ErrorCode::InputSchema
    );
}
