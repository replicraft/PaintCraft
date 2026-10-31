//! Immutable resolved schemas for the current eleven-profile catalog.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "catalog" => include_str!(
            "../../schemas/catalog/5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b/manifest.json"
        ),
        "batch-catalog" => include_str!(
            "../../schemas/catalog/5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b/batch.schema.json"
        ),
        "result-catalog" => include_str!(
            "../../schemas/catalog/5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b/result.schema.json"
        ),
        "file-command-catalog" => include_str!(
            "../../schemas/catalog/5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b/file-command.schema.json"
        ),
        "inspect-catalog" => include_str!(
            "../../schemas/catalog/5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b/inspect.schema.json"
        ),
        "mcp-inspect-catalog" => include_str!(
            "../../schemas/catalog/5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
