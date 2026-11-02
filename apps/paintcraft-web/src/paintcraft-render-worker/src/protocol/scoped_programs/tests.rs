use super::*;
use paintcraft_render::noise_work::NoiseJobPolicy;
fn workload() -> Workload {
    Workload::browser(NoiseJobPolicy::BrowserV1.max_limits()).unwrap()
}
fn subset(ids: &[&str]) -> Requirements {
    let mut r = Requirements::default();
    for id in ids {
        r.insert(id, "params-v1").unwrap();
    }
    r
}
fn pair_value(required: &Requirements) -> Value {
    Value::Array(
        required
            .pairs()
            .map(|(profile, codec)| {
                Value::object([
                    ("profile_id", Value::str(profile)),
                    ("codec_id", Value::str(codec)),
                ])
            })
            .collect(),
    )
}
fn full(required: &Requirements) -> Value {
    let Value::Object(mut fields) = Fact::capture(required, workload(), &|| Ok(()))
        .unwrap()
        .value()
    else {
        unreachable!()
    };
    fields.insert("pairs".into(), pair_value(required));
    Value::Object(fields)
}
fn replace(mut value: Value, key: &str, replacement: Value) -> Value {
    let Value::Object(ref mut fields) = value else {
        unreachable!()
    };
    fields.insert(key.into(), replacement);
    value
}
fn source(required: &Requirements) -> Value {
    Value::object([
        (
            "owner",
            Value::object([
                (
                    "session",
                    Value::str("00000000-0000-0000-0000-000000000001"),
                ),
                ("state", Value::str("00000000-0000-0000-0000-000000000002")),
                ("epoch", Value::str("0")),
                ("frontend", Value::Int(3)),
            ]),
        ),
        ("draft", Value::str("00000000-0000-0000-0000-000000000003")),
        (
            "document",
            Value::str("00000000-0000-0000-0000-000000000004"),
        ),
        ("sourceSha256", Value::str("a".repeat(64))),
        ("filterDigest", Value::str("b".repeat(64))),
        ("size", Value::Array(vec![Value::Int(2), Value::Int(1)])),
        ("layers", Value::Int(1)),
        ("residentBytes", Value::Int(0)),
        ("metadataBytes", Value::Int(0)),
        ("metadataCharge", Value::Int(0)),
        ("maskCount", Value::Int(0)),
        ("maskBands", Value::Int(0)),
        ("maskRuns", Value::Int(0)),
        ("maskDecodeBytes", Value::Int(0)),
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
        ("sourceRequirements", full(required)),
        ("workload", workload().value().unwrap()),
    ])
}
fn request(source: &Value, required: &Requirements) -> Value {
    Value::object([
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
        ("owner", field(source, "owner").unwrap().clone()),
        ("draft", field(source, "draft").unwrap().clone()),
        (
            "sourceSha256",
            field(source, "sourceSha256").unwrap().clone(),
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
        (
            "candidateRequirements",
            Fact::capture(required, workload(), &|| Ok(()))
                .unwrap()
                .value(),
        ),
        (
            "unionRequirements",
            Fact::capture(required, workload(), &|| Ok(()))
                .unwrap()
                .value(),
        ),
        (
            "workloadSha256",
            Value::str(hex(&workload().digest().unwrap())),
        ),
    ])
}
#[test]
fn hello2_returns_only_resolved_subset_with_class_and_exact_workload_binding() {
    let required = subset(&["add-noise-rgb-v1", "invert-rgb-v1"]);
    let query = Value::object([
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
        ("pairs", pair_value(&required)),
        ("workload", workload().value().unwrap()),
    ]);
    let reply = hello(&text(&query), &|| Ok(())).unwrap();
    let entries = field(&reply, "entries").unwrap().as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        string(field(&entries[0], "workload_class_id").unwrap()).unwrap(),
        "noise-source-work-v1"
    );
    assert_eq!(
        string(field(&entries[1], "workload_class_id").unwrap()).unwrap(),
        "base-work-v1"
    );
    let fact = Fact::capture(&required, workload(), &|| Ok(())).unwrap();
    assert_eq!(
        digest(field(&reply, "requiredSha256").unwrap()).unwrap(),
        hex(&fact.base.required)
    );
    assert_eq!(
        digest(field(&reply, "resolutionSha256").unwrap()).unwrap(),
        hex(&fact.base.resolution)
    );
    assert_eq!(
        digest(field(&reply, "workloadSha256").unwrap()).unwrap(),
        hex(&workload().digest().unwrap())
    );
    assert!(text(&reply).len() <= CAPABILITY_BYTES_MAX);
    for (key, bad) in [
        ("registryContract", Value::Int(1)),
        ("registryContract", Value::Int(4)),
        ("protocol", Value::Int(5)),
    ] {
        assert_eq!(
            hello(&text(&replace(query.clone(), key, bad)), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        super::super::programs::requirements(&pair_value(&required), 257)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let legacy_query = Value::object([
        ("registryContract", Value::Int(1)),
        ("pairs", pair_value(&required)),
    ]);
    assert_eq!(
        super::super::programs::hello(&text(&legacy_query))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn open6_has_one_workload_authority_and_strict_declared_requirement_facts() {
    let r = subset(&["add-noise-rgb-v1"]);
    let value = source(&r);
    let (parsed, profile) = open(&text(&value), &|| Ok(())).unwrap();
    assert_eq!(
        profile.source,
        Fact::capture(&r, workload(), &|| Ok(())).unwrap()
    );
    assert_eq!(profile.workload, workload());
    assert_eq!(parsed.value, value);
    for key in ["workspaceBytes", "workLimit"] {
        assert!(
            open(&text(&replace(value.clone(), key, Value::Int(1))), &|| Ok(
                ()
            ))
            .is_err()
        );
    }
    let bad = replace(full(&r), "count", Value::Int(0));
    assert_eq!(
        open(
            &text(&replace(value.clone(), "sourceRequirements", bad)),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
    let bad = replace(workload().value().unwrap(), "noiseWorkLimit", Value::Int(1));
    assert!(open(&text(&replace(value, "workload", bad)), &|| Ok(())).is_err());
}
#[test]
fn begin6_binds_source_and_workload_without_reparse_or_execution() {
    let r = subset(&["add-noise-rgb-v1"]);
    let src = source(&r);
    let (parsed, profile) = open(&text(&src), &|| Ok(())).unwrap();
    let packet = request(&src, &r);
    let parsed_begin = begin(&text(&packet), &|| Ok(())).unwrap();
    assert_eq!(parsed_begin.value(), &packet);
    assert_eq!(parsed_begin.generation, 1);
    assert!(!parsed_begin.render);
    parsed_begin.matches_source(&parsed, profile).unwrap();
    let wrong = begin(
        &text(&replace(
            packet.clone(),
            "workloadSha256",
            Value::str("c".repeat(64)),
        )),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        wrong.matches_source(&parsed, profile).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    for (key, bad) in [
        ("generation", Value::str("01")),
        ("render", Value::Int(0)),
        ("target", Value::str("00000000-0000-0000-0000-000000000004")),
    ] {
        assert!(begin(&text(&replace(packet.clone(), key, bad)), &|| Ok(())).is_err());
    }
    assert_eq!(parsed_begin.into_value(), packet);
}
#[test]
fn controls_pairs_operation_and_cancellation_have_independent_bounds() {
    assert_eq!(
        hello(&" ".repeat(CAPABILITY_BYTES_MAX + 1), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        open(&" ".repeat(OPEN_BYTES_MAX + 1), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        begin(&" ".repeat(BEGIN_BYTES_MAX + 1), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let duplicate = pair_value(&subset(&["invert-rgb-v1"]));
    let Value::Array(mut rows) = duplicate else {
        unreachable!()
    };
    rows.push(rows[0].clone());
    assert_eq!(
        requirements(&Value::Array(rows), 257).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        requirements(&Value::Array(vec![]), 258).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let unknown = Value::Array(vec![Value::object([
        ("profile_id", Value::str("unknown-v1")),
        ("codec_id", Value::str("params-v1")),
    ])]);
    assert_eq!(
        requirements(&unknown, 257).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let r = subset(&[]);
    let src = source(&r);
    let packet = request(&src, &r);
    let huge = Value::object([
        ("op", Value::str("filter.add")),
        ("padding", Value::str("x".repeat(4096))),
    ]);
    assert_eq!(
        begin(&text(&replace(packet, "operation", huge)), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        open(&text(&src), &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "stop"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}

#[test]
fn independent_literal_capability_vectors_bind_class_workload_and_legacy_hashes() {
    use paintcraft_render::noise_work::NoiseLimits;
    {
        let required = subset(&[]);
        let workload = Workload::browser(NoiseLimits {
            bytes: 67108864,
            base_work: 268435456,
            noise_work: 34359738368,
        })
        .unwrap();
        let fact = Fact::capture(&required, workload, &|| Ok(())).unwrap();
        assert_eq!(
            hex(&fact.base.required),
            "ded9f6320cea35d29463746a3ee5f7c94ec0d8f275bab2bd3c8d0911169da840"
        );
        assert_eq!(
            hex(&fact.base.resolution),
            "f9eb50503220d9d69a61610c4ad9e54553219e8844ce8b19bba08b7de80044d7"
        );
        assert_eq!(
            hex(&fact.capability),
            "e3f60bb6f0dd7e24a208ce61eafd86323c43383d48f56b218baeed33a2a7e10b"
        );
        assert_eq!(
            hex(&workload.digest().unwrap()),
            "66f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fd"
        );
        let parsed = Fact::parse(&fact.value(), 257, false).unwrap();
        parsed.matches(&required, workload, &|| Ok(())).unwrap();
        let changed = replace(fact.value(), "capabilitySha256", Value::str("0".repeat(64)));
        assert_eq!(
            Fact::parse(&changed, 257, false)
                .unwrap()
                .matches(&required, workload, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
    {
        let required = subset(&["add-noise-rgb-v1"]);
        let workload = Workload::browser(NoiseLimits {
            bytes: 1048576,
            base_work: 1048576,
            noise_work: 8388608,
        })
        .unwrap();
        let fact = Fact::capture(&required, workload, &|| Ok(())).unwrap();
        assert_eq!(
            hex(&fact.base.required),
            "290d54fb7465ca9a7082a81caec5877a2c193533000765a4f4cc3c2af2a1a1d3"
        );
        assert_eq!(
            hex(&fact.base.resolution),
            "707c80ef7af987cea18668d72f78a1018cc958a51d815a668d4b7dbdb6803261"
        );
        assert_eq!(
            hex(&fact.capability),
            "926795e25d1b2d7355d78c5d89b39b09f17a54f30b450a48ae29c593c83d8dc6"
        );
        assert_eq!(
            hex(&workload.digest().unwrap()),
            "1f9a9b9e7b7c658d4ea1448315561e22aa9ae04bbc2659120c74962930828ad9"
        );
        let parsed = Fact::parse(&fact.value(), 257, false).unwrap();
        parsed.matches(&required, workload, &|| Ok(())).unwrap();
        let changed = replace(fact.value(), "capabilitySha256", Value::str("0".repeat(64)));
        assert_eq!(
            Fact::parse(&changed, 257, false)
                .unwrap()
                .matches(&required, workload, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
    {
        let required = subset(&["add-noise-rgb-v1", "invert-rgb-v1"]);
        let workload = Workload::browser(NoiseLimits {
            bytes: 67108864,
            base_work: 268435456,
            noise_work: 34359738368,
        })
        .unwrap();
        let fact = Fact::capture(&required, workload, &|| Ok(())).unwrap();
        assert_eq!(
            hex(&fact.base.required),
            "55648214762fa48c9e8c1470e6f8a46343b46c8f66df193028278ad39b219a74"
        );
        assert_eq!(
            hex(&fact.base.resolution),
            "209af4ec3e29790211b135631926a9c9203a86ca0f50253a71a140f3db1b4395"
        );
        assert_eq!(
            hex(&fact.capability),
            "af8ab21a88343a5d050f8fc947550f5ebf4b990ce887669eedb3616935160ff2"
        );
        assert_eq!(
            hex(&workload.digest().unwrap()),
            "66f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fd"
        );
        let parsed = Fact::parse(&fact.value(), 257, false).unwrap();
        parsed.matches(&required, workload, &|| Ok(())).unwrap();
        let changed = replace(fact.value(), "capabilitySha256", Value::str("0".repeat(64)));
        assert_eq!(
            Fact::parse(&changed, 257, false)
                .unwrap()
                .matches(&required, workload, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
    {
        let required = subset(&[
            "black-white-rgb-v1",
            "box-blur-v1",
            "brightness-contrast-v1",
            "curves-rgb-v1",
            "exposure-srgb-v1",
            "hue-saturation-rgb-v1",
            "invert-rgb-v1",
            "levels-rgb-v1",
            "posterize-rgb-v1",
            "solarize-rgb-v1",
            "threshold-luma-v1",
            "vibrance-rgb-v1",
        ]);
        let workload = Workload::browser(NoiseLimits {
            bytes: 67108864,
            base_work: 268435456,
            noise_work: 34359738368,
        })
        .unwrap();
        let fact = Fact::capture(&required, workload, &|| Ok(())).unwrap();
        assert_eq!(
            hex(&fact.base.required),
            "585a38dbdd4ff739357095aba8ca932ef90785942361e7335b772de331e553c1"
        );
        assert_eq!(
            hex(&fact.base.resolution),
            "19c1306ec948ec8a0ae131be6fe4418e8bb9fc805b41c3bbfbbb6d4bd0539c1b"
        );
        assert_eq!(
            hex(&fact.capability),
            "1039dcd925af67b6da9cfd0923b15f34ca46d0e4aa39a8acd4e62cdc17f12774"
        );
        assert_eq!(
            hex(&workload.digest().unwrap()),
            "66f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fd"
        );
        let parsed = Fact::parse(&fact.value(), 257, false).unwrap();
        parsed.matches(&required, workload, &|| Ok(())).unwrap();
        let changed = replace(fact.value(), "capabilitySha256", Value::str("0".repeat(64)));
        assert_eq!(
            Fact::parse(&changed, 257, false)
                .unwrap()
                .matches(&required, workload, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
}
