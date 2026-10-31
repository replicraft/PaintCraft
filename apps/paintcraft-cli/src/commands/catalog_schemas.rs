//! Immutable resolved schemas for the current ten-profile catalog.
pub(super) fn schema(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "catalog" => include_str!(
            "../../schemas/catalog/2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/manifest.json"
        ),
        "batch-catalog" => include_str!(
            "../../schemas/catalog/2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/batch.schema.json"
        ),
        "result-catalog" => include_str!(
            "../../schemas/catalog/2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/result.schema.json"
        ),
        "file-command-catalog" => include_str!(
            "../../schemas/catalog/2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/file-command.schema.json"
        ),
        "inspect-catalog" => include_str!(
            "../../schemas/catalog/2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/inspect.schema.json"
        ),
        "mcp-inspect-catalog" => include_str!(
            "../../schemas/catalog/2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/mcp-inspect.schema.json"
        ),
        _ => return None,
    })
}
