//! Read-only source descriptors. No editor/session state is consulted or changed.
use super::{PaintingServer, native_codec::RpcLife, state::core_json};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{path::PathBuf, sync::atomic::Ordering};
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspect {
    path: String,
    expect_source_sha256: Option<String>,
    max_input_bytes: Option<u64>,
    max_pixels: Option<u64>,
    /// Adapter-specific metadata/ICC allowance, not total memory or fixed overhead.
    max_metadata_bytes: Option<u64>,
}
#[tool_router(router=source_inspection_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Inspect original PNG, BMP or JPEG source descriptors without decoding pixels or changing a document. Reports raw SHA, geometry, palette/color/metadata facts and proposed conversion losses. This report is not import authorization or a concurrency ticket. Other formats are unsupported by this tool.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn inspect_source(
        &self,
        Parameters(p): Parameters<Inspect>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = RpcLife::new();
        let dropped = life.0.clone();
        let cancel = context.ct.clone();
        self.work(move |_| {
            let check = || {
                if cancel.is_cancelled() || dropped.load(Ordering::Acquire) {
                    Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::OperationCancelled,
                        "Source inspection cancelled",
                    ))
                } else {
                    Ok(())
                }
            };
            let defaults = paintcraft_formats::source_inspection::Limits::default();
            let (value, _) = super::super::inspect_source::inspect_path(
                &PathBuf::from(p.path),
                p.expect_source_sha256.as_deref(),
                paintcraft_formats::source_inspection::Limits {
                    input_bytes: p.max_input_bytes.unwrap_or(defaults.input_bytes),
                    pixels: p.max_pixels.unwrap_or(defaults.pixels),
                    metadata_bytes: p.max_metadata_bytes,
                },
                &check,
            )?;
            Ok(CallToolResult::success(vec![ContentBlock::text(
                core_json(&value)?.to_string(),
            )]))
        })
        .await
    }
}

#[cfg(test)]
#[path = "source_inspection/tests.rs"]
mod tests;
