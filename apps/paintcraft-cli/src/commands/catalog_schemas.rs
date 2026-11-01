//! Immutable resolved schemas for the current thirteen-profile catalog.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "catalog" => include_str!(
            "../../schemas/catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/manifest.json"
        ),
        "batch-catalog" => include_str!(
            "../../schemas/catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/batch.schema.json"
        ),
        "result-catalog" => include_str!(
            "../../schemas/catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/result.schema.json"
        ),
        "file-command-catalog" => include_str!(
            "../../schemas/catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/file-command.schema.json"
        ),
        "inspect-catalog" => include_str!(
            "../../schemas/catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/inspect.schema.json"
        ),
        "mcp-inspect-catalog" => include_str!(
            "../../schemas/catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
