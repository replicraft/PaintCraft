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

/// `paintcraft.file-command/v1`: the engine result and the external file receipt,
/// reported independently.
pub fn envelope(engine: Option<Value>, file: Option<Value>, outcome: &str, exit_code: u8) -> Value {
    Value::object([
        ("schema", Value::str("paintcraft.file-command/v1")),
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
