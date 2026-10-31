use super::*;
fn changed(mut value: Value, key: &str, replacement: Value) -> Value {
    let Value::Object(ref mut fields) = value else {
        panic!("object fixture")
    };
    fields.insert(key.into(), replacement);
    value
}
fn removed(mut value: Value, key: &str) -> Value {
    let Value::Object(ref mut fields) = value else {
        panic!("object fixture")
    };
    fields.remove(key);
    value
}
#[test]
fn explicit_binary_binding_matches_independent_literal_vectors() {
    // Independently encoded with Python stdlib struct.pack('>IQQQQ', ...)
    // and hashlib over the documented domain + u16BE id length + id bytes.
    for (bytes, base, noise, expected) in [
        (
            64 << 20,
            268_435_456,
            1 << 35,
            "66f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fd",
        ),
        (
            0,
            0,
            0,
            "443ae9ac60767a7680aea81c07c83b3979334111b8ab32ef56f16d2c8b033f50",
        ),
        (
            1024,
            10,
            20,
            "1592851cdf81c6ebfa1011517a8217ed8d5961c96e60962a067893ff81346185",
        ),
    ] {
        let workload = Workload::browser(NoiseLimits {
            bytes,
            base_work: base,
            noise_work: noise,
        })
        .unwrap();
        assert_eq!(hex(&workload.digest().unwrap()), expected);
        let value = workload.value().unwrap();
        assert_eq!(value.as_object().unwrap().len(), 7);
        assert_eq!(
            value.get("totalWorkLimit").unwrap().as_i64(),
            Some((base + noise) as i64)
        );
        assert_eq!(Workload::parse_browser(&value).unwrap(), workload);
        assert_eq!(
            workload.limits(),
            NoiseLimits {
                bytes,
                base_work: base,
                noise_work: noise
            }
        );
    }
}
#[test]
fn browser_wire_cannot_select_native_limits_or_exceed_either_component() {
    assert_eq!(
        Workload::browser(NoiseJobPolicy::NativeV1.max_limits())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let max = NoiseJobPolicy::BrowserV1.max_limits();
    assert_eq!(
        max,
        NoiseLimits {
            bytes: 64 << 20,
            base_work: 268_435_456,
            noise_work: 1 << 35,
        }
    );
    for limits in [
        NoiseLimits {
            bytes: max.bytes + 1,
            ..max
        },
        NoiseLimits {
            base_work: max.base_work + 1,
            ..max
        },
        NoiseLimits {
            noise_work: max.noise_work + 1,
            ..max
        },
        NoiseLimits {
            base_work: u64::MAX,
            noise_work: 1,
            ..max
        },
    ] {
        assert_eq!(
            Workload::browser(limits).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    let workload = Workload::browser(max).unwrap();
    assert_eq!(
        workload
            .check(WorkComponents {
                base: max.base_work + 1,
                noise: 0
            })
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        workload
            .check(WorkComponents {
                base: 0,
                noise: max.noise_work + 1
            })
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn every_workload_field_is_required_and_typed_and_binding_is_not_authority() {
    let workload = Workload::browser(NoiseJobPolicy::BrowserV1.max_limits()).unwrap();
    let value = workload.value().unwrap();
    for key in [
        "id",
        "revision",
        "workspaceBytes",
        "baseWorkLimit",
        "noiseWorkLimit",
        "totalWorkLimit",
        "sha256",
    ] {
        assert_eq!(
            Workload::parse_browser(&removed(value.clone(), key))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        for wrong in [Value::Null, Value::Bool(true)] {
            assert_eq!(
                Workload::parse_browser(&changed(value.clone(), key, wrong))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
    }
    assert_eq!(
        Workload::parse_browser(&changed(value.clone(), "extra", Value::Int(0)))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    for key in [
        "workspaceBytes",
        "baseWorkLimit",
        "noiseWorkLimit",
        "totalWorkLimit",
    ] {
        for wrong in [Value::Int(-1), Value::str("1")] {
            assert_eq!(
                Workload::parse_browser(&changed(value.clone(), key, wrong))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
    }
    for (key, wrong) in [
        ("id", Value::str("noise-source-work-v2")),
        ("revision", Value::Int(2)),
    ] {
        assert_eq!(
            Workload::parse_browser(&changed(value.clone(), key, wrong))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        Workload::parse_browser(&changed(value.clone(), "totalWorkLimit", Value::Int(0)))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        Workload::parse_browser(&changed(
            value.clone(),
            "sha256",
            Value::str("0".repeat(64))
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    // Recomputing a digest is never sufficient to authorize a larger base limit.
    let wrong = changed(value, "baseWorkLimit", Value::Int(1_000_000_000));
    assert_eq!(
        Workload::parse_browser(&wrong).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn quote_facts_are_bound_to_exact_limits_and_cannot_exchange_components() {
    let workload = Workload::browser(NoiseLimits {
        bytes: 1024,
        base_work: 10,
        noise_work: 20,
    })
    .unwrap();
    let quote = WorkComponents {
        base: 10,
        noise: 20,
    };
    let value = workload.quote_value(quote).unwrap();
    assert_eq!(workload.parse_quote(&value).unwrap(), quote);
    assert_eq!(
        workload
            .parse_quote(&changed(value.clone(), "base", Value::Int(11)))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        workload
            .parse_quote(&changed(value.clone(), "noise", Value::Int(21)))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        workload
            .parse_quote(&changed(value.clone(), "total", Value::Int(29)))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        workload
            .parse_quote(&changed(value.clone(), "extra", Value::Int(0)))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    for key in ["workloadSha256", "base", "noise", "total"] {
        assert_eq!(
            workload
                .parse_quote(&removed(value.clone(), key))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let another = Workload::browser(NoiseJobPolicy::BrowserV1.max_limits()).unwrap();
    assert_eq!(
        another.parse_quote(&value).unwrap_err().code,
        ErrorCode::InputSchema
    );
    let zero = WorkComponents::default();
    assert_eq!(
        workload
            .parse_quote(&workload.quote_value(zero).unwrap())
            .unwrap(),
        zero
    );
}
