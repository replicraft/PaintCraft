//! Immutable resolved schemas for the initial eight-profile catalog.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "catalog" => include_str!(
            "../../schemas/catalog/d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/manifest.json"
        ),
        "batch-catalog" => include_str!(
            "../../schemas/catalog/d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/batch.schema.json"
        ),
        "result-catalog" => include_str!(
            "../../schemas/catalog/d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/result.schema.json"
        ),
        "file-command-catalog" => include_str!(
            "../../schemas/catalog/d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/file-command.schema.json"
        ),
        "inspect-catalog" => include_str!(
            "../../schemas/catalog/d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/inspect.schema.json"
        ),
        "mcp-inspect-catalog" => include_str!(
            "../../schemas/catalog/d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
