//! Immutable resolved schemas for the current nine-profile catalog.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "catalog" => include_str!(
            "../../schemas/catalog/cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/manifest.json"
        ),
        "batch-catalog" => include_str!(
            "../../schemas/catalog/cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/batch.schema.json"
        ),
        "result-catalog" => include_str!(
            "../../schemas/catalog/cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/result.schema.json"
        ),
        "file-command-catalog" => include_str!(
            "../../schemas/catalog/cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/file-command.schema.json"
        ),
        "inspect-catalog" => include_str!(
            "../../schemas/catalog/cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/inspect.schema.json"
        ),
        "mcp-inspect-catalog" => include_str!(
            "../../schemas/catalog/cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
