//! Additive immutable layer appearance; historical catalog aliases stay fixed.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "layer-appearance" => include_str!(
            "../../schemas/layer-appearance/8b05904f87725d78058f48a148e0af2ab0a2536dc369c052179edffa86e0c676/manifest.json"
        ),
        "batch-layer-appearance" => include_str!(
            "../../schemas/layer-appearance/8b05904f87725d78058f48a148e0af2ab0a2536dc369c052179edffa86e0c676/batch.schema.json"
        ),
        "result-layer-appearance" => include_str!(
            "../../schemas/layer-appearance/8b05904f87725d78058f48a148e0af2ab0a2536dc369c052179edffa86e0c676/result.schema.json"
        ),
        "file-command-layer-appearance" => include_str!(
            "../../schemas/layer-appearance/8b05904f87725d78058f48a148e0af2ab0a2536dc369c052179edffa86e0c676/file-command.schema.json"
        ),
        "inspect-v5" => include_str!(
            "../../schemas/layer-appearance/8b05904f87725d78058f48a148e0af2ab0a2536dc369c052179edffa86e0c676/inspect.schema.json"
        ),
        "mcp-inspect-v5" => include_str!(
            "../../schemas/layer-appearance/8b05904f87725d78058f48a148e0af2ab0a2536dc369c052179edffa86e0c676/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
