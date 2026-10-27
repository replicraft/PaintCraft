use super::*;
use paintcraft_core::types::json::Value;
use paintcraft_formats::codec_worker::{self as wire, jxr_encode::qualified};
#[test]
fn native_jxr_discovery_is_exact_studio_only_and_requires_installation_preflight() {
    for frontend in [
        None,
        Some("studio"),
        Some("classic-v7"),
        Some("classic-v10"),
        Some("classic-v11"),
    ] {
        let mut args = vec!["--json", "capabilities"];
        if let Some(frontend) = frontend {
            args.extend(["--frontend", frontend]);
        }
        let output = run(&args);
        assert!(output.status.success());
        let value = json::parse(&output.stdout).unwrap();
        let rows = value.get("capabilities").and_then(Value::as_array).unwrap();
        for (variant, capability, operation) in [
            (
                qualified::WRITER_IMPORT_VARIANT,
                "jxr.writer-rgb8.import",
                "import",
            ),
            (
                qualified::EXPORT_VARIANT,
                "jxr.rgb8.lossless.write",
                "fresh-write",
            ),
        ] {
            let found: Vec<_> = rows
                .iter()
                .filter(|row| row.get("variant").and_then(Value::as_str) == Some(variant))
                .collect();
            assert_eq!(
                found.len(),
                usize::from(frontend.is_none_or(|value| value == "studio"))
            );
            if let Some(row) = found.first() {
                for (key, expected) in [
                    ("capability", capability),
                    ("operation", operation),
                    ("variant", variant),
                    ("profile", "frontend/studio"),
                    ("platform", "native"),
                    ("backend_id", "jpegxr-pure-rs/native-worker"),
                    ("required_helper_profile", qualified::NATIVE_PROFILE),
                    ("evidence", "unverified"),
                    ("result_status", "not-run"),
                ] {
                    assert_eq!(
                        row.get(key).and_then(Value::as_str),
                        Some(expected),
                        "{key}"
                    );
                }
                for (key, expected) in [
                    ("installation_preflight_required", true),
                    ("runtime_installation_verified", false),
                    ("required_helper_profile_is_minimum", false),
                    ("release_qualified", false),
                ] {
                    assert_eq!(
                        row.get(key).and_then(Value::as_bool),
                        Some(expected),
                        "{key}"
                    );
                }
                let accepted: Vec<_> = row
                    .get("accepted_helper_profiles")
                    .and_then(Value::as_array)
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap())
                    .collect();
                assert_eq!(accepted, [qualified::NATIVE_PROFILE]);
            }
        }
        assert!(
            !rows.iter().any(|row| matches!(
                row.get("capability").and_then(Value::as_str),
                Some("jxr.rgba8.read" | "jxr.rgba8.write")
            )),
            "named variants cannot advertise generic JXR Open/write"
        );
    }
}
#[test]
fn established_native_import_rows_list_exact_legacy_and_public_v8_contracts() {
    let output = run(&["--json", "capabilities", "--frontend", "studio"]);
    assert!(output.status.success());
    let value = json::parse(&output.stdout).unwrap();
    let rows = value.get("capabilities").and_then(Value::as_array).unwrap();
    for variant in [
        wire::NATIVE_JXL_IMPORT_VARIANT,
        wire::NATIVE_JXR_IMPORT_VARIANT,
        wire::NATIVE_HEIF_IMPORT_VARIANT,
    ] {
        let row = rows
            .iter()
            .find(|row| row.get("variant").and_then(Value::as_str) == Some(variant))
            .unwrap();
        let accepted: Vec<_> = row
            .get("accepted_helper_profiles")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(
            accepted,
            [
                wire::NATIVE_PROFILE,
                wire::AVIF_NATIVE_PROFILE,
                wire::AVIF_SDR_NATIVE_PROFILE,
                wire::FLOAT_NATIVE_PROFILE,
                qualified::NATIVE_PROFILE
            ]
        );
        assert!(!accepted.contains(&wire::jxr_encode::candidate::NATIVE_PROFILE));
        assert_eq!(
            row.get("runtime_installation_verified")
                .and_then(Value::as_bool),
            Some(false)
        );
    }
}
