//! JSON reporting: error/v1, the file-command envelope and printing.

use paintcraft_core::types::Error;
use paintcraft_core::types::json::{self, Value};

pub fn error_value(e: &Error) -> Value {
    let mut pairs = vec![
        ("schema", Value::str("paintcraft.error/v1")),
        ("committed", Value::Bool(false)),
        ("code", Value::str(e.code.as_str())),
        (
            "message",
            Value::str(e.message.chars().take(4096).collect::<String>()),
        ),
    ];
    if let Some(i) = e.operation_index {
        pairs.push(("operation_index", Value::Int(i as i64)));
    }
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// File-command/v5 carries own-layer-mask result/v5; older results retain their exact profile.
/// The engine result and external file receipt are
/// reported independently.
pub fn envelope(engine: Option<Value>, file: Option<Value>, outcome: &str, exit_code: u8) -> Value {
    let engine_schema = engine
        .as_ref()
        .and_then(|value| value.get("schema"))
        .and_then(Value::as_str);
    Value::object([
        (
            "schema",
            Value::str(
                if matches!(
                    engine_schema,
                    Some(
                        "paintcraft.result/layer-styles-v1"
                            | "paintcraft.export-disclosure/layer-styles-v1"
                    )
                ) {
                    "paintcraft.file-command/layer-styles-v1"
                } else if engine_schema == Some("paintcraft.result/v6") {
                    "paintcraft.file-command/v6"
                } else if engine_schema == Some("paintcraft.result/v5") {
                    "paintcraft.file-command/v5"
                } else if engine_schema == Some("paintcraft.result/catalog-v1") {
                    "paintcraft.file-command/catalog-v1"
                } else if engine_schema == Some("paintcraft.result/v4") {
                    "paintcraft.file-command/v4"
                } else if engine_schema == Some("paintcraft.result/v3") {
                    "paintcraft.file-command/v3"
                } else if engine_schema == Some("paintcraft.result/v2") {
                    "paintcraft.file-command/v2"
                } else {
                    "paintcraft.file-command/v1"
                },
            ),
        ),
        ("engine", engine.unwrap_or(Value::Null)),
        ("file", file.unwrap_or(Value::Null)),
        ("outcome", Value::str(outcome)),
        ("exit_code", Value::Int(i64::from(exit_code))),
        ("synthetic", Value::Bool(false)),
    ])
}

/// One newline-terminated JSON object on stdout.
pub fn json_line(v: &Value) -> Vec<u8> {
    let mut b = json::canonical(v);
    b.push(b'\n');
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{
        ErrorCode,
        resources::{AllocationClass, SharedBudget},
    };
    #[test]
    fn allocation_diagnostics_do_not_change_error_v1_fields_or_canonical_bytes() {
        let budget = SharedBudget::new(0);
        let error = budget
            .reserve(1)
            .unwrap_err()
            .with_allocation_requirement(AllocationClass::DecodedTiles, 2)
            .at(4);
        assert!(error.allocation_denial().is_some());
        let old = Error::new(ErrorCode::ResourceLimit, "shared byte budget is full").at(4);
        assert_eq!(
            json_line(&error_value(&error)),
            json_line(&error_value(&old))
        );
        let Value::Object(fields) = error_value(&error) else {
            panic!("error object")
        };
        assert_eq!(fields.len(), 5);
    }
}

#[cfg(test)]
mod layer_mask_profile_tests {
    use super::*;
    #[test]
    fn own_masks_use_v5_without_downgrading_any_existing_engine_profile() {
        for (engine_schema, outer) in [
            ("paintcraft.result/v6", "paintcraft.file-command/v6"),
            ("paintcraft.result/v5", "paintcraft.file-command/v5"),
            (
                "paintcraft.result/catalog-v1",
                "paintcraft.file-command/catalog-v1",
            ),
            ("paintcraft.result/v4", "paintcraft.file-command/v4"),
            ("paintcraft.result/v3", "paintcraft.file-command/v3"),
            ("paintcraft.result/v2", "paintcraft.file-command/v2"),
            ("paintcraft.result/v1", "paintcraft.file-command/v1"),
        ] {
            let engine = Value::object([("schema", Value::str(engine_schema))]);
            let reply = envelope(Some(engine.clone()), None, "ok", 0);
            assert_eq!(reply.get("schema").and_then(Value::as_str), Some(outer));
            assert_eq!(reply.get("engine"), Some(&engine));
            assert_eq!(reply.get("file"), Some(&Value::Null));
        }
    }
}

#[cfg(test)]
#[path = "report/blend_schema_tests.rs"]
mod blend_schema_tests;
