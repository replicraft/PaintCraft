//! Immutable resolved schemas for the current twelve-profile catalog.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "catalog" => include_str!(
            "../../schemas/catalog/a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a/manifest.json"
        ),
        "batch-catalog" => include_str!(
            "../../schemas/catalog/a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a/batch.schema.json"
        ),
        "result-catalog" => include_str!(
            "../../schemas/catalog/a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a/result.schema.json"
        ),
        "file-command-catalog" => include_str!(
            "../../schemas/catalog/a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a/file-command.schema.json"
        ),
        "inspect-catalog" => include_str!(
            "../../schemas/catalog/a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a/inspect.schema.json"
        ),
        "mcp-inspect-catalog" => include_str!(
            "../../schemas/catalog/a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
