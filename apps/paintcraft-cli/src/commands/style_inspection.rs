//! Explicit style inspection/discovery. Descriptive values are never bake grants.
use paintcraft_core::types::json::Value;
pub(super) fn blend(mode: paintcraft_core::types::LayerBlendMode) -> &'static str {
    match mode {
        paintcraft_core::types::LayerBlendMode::Normal => "normal",
        paintcraft_core::types::LayerBlendMode::Multiply => "multiply",
        paintcraft_core::types::LayerBlendMode::Screen => "screen",
    }
}
pub(super) fn stack(stack: &paintcraft_document::StyleStack) -> Value {
    Value::Array(
        stack
            .iter()
            .map(|s| {
                Value::object([
                    ("id", Value::str(s.id().to_string())),
                    ("enabled", Value::Bool(s.enabled())),
                    ("descriptor", s.descriptor().to_value()),
                ])
            })
            .collect(),
    )
}
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "layer-styles" => include_str!("../../schemas/layer-styles-v1/manifest.json"),
        "batch-layer-styles" => include_str!("../../schemas/layer-styles-v1/batch.schema.json"),
        "result-layer-styles" => include_str!("../../schemas/layer-styles-v1/result.schema.json"),
        "file-command-layer-styles" => {
            include_str!("../../schemas/layer-styles-v1/file-command.schema.json")
        }
        "inspect-layer-styles" => include_str!("../../schemas/layer-styles-v1/inspect.schema.json"),
        "mcp-inspect-layer-styles" => {
            include_str!("../../schemas/layer-styles-v1/mcp-inspect.schema.json")
        }
        "export-disclosure-layer-styles" => {
            include_str!("../../schemas/layer-styles-v1/export-disclosure.schema.json")
        }
        _ => return None,
    })
}
