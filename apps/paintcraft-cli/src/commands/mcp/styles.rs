//! Style-specific schema discovery; commands still use shared atomic apply_operations.
use super::PaintingServer;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    #[default]
    Manifest,
    Batch,
    Result,
    FileCommand,
    Inspect,
    McpInspect,
    ExportDisclosure,
}
impl Kind {
    fn name(&self) -> &'static str {
        match self {
            Self::Manifest => "layer-styles",
            Self::Batch => "batch-layer-styles",
            Self::Result => "result-layer-styles",
            Self::FileCommand => "file-command-layer-styles",
            Self::Inspect => "inspect-layer-styles",
            Self::McpInspect => "mcp-inspect-layer-styles",
            Self::ExportDisclosure => "export-disclosure-layer-styles",
        }
    }
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Schema {
    #[serde(default)]
    kind: Kind,
}
#[tool_router(router=style_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Get the native CLI/MCP layer-styles-v1 schemas for six ordered Color Overlay operations, PCND9 inspection and compound styled PNG disclosure. Discovery grants no bake, browser or GPU authority."
    )]
    async fn get_layer_style_schema(&self, Parameters(p): Parameters<Schema>) -> CallToolResult {
        let text = super::super::style_inspection::schema(p.kind.name())
            .expect("closed style schema kind");
        CallToolResult::success(vec![ContentBlock::text(text)])
    }
}
