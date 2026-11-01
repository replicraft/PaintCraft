//! Typed controls over actual Session selection; no numeric mask or new realm.
use super::PaintingServer;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Enabled {
    /// Defaults to the current top layer; any supplied ID must be a strict UUID.
    layer_id: Option<String>,
    enabled: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Layer {
    layer_id: Option<String>,
}
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
}
impl Kind {
    fn name(&self) -> &'static str {
        match self {
            Self::Manifest => "layer-appearance",
            Self::Batch => "batch-layer-appearance",
            Self::Result => "result-layer-appearance",
            Self::FileCommand => "file-command-layer-appearance",
            Self::Inspect => "inspect-v5",
            Self::McpInspect => "mcp-inspect-v5",
        }
    }
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Schema {
    #[serde(default)]
    kind: Kind,
}
#[tool_router(router=layer_mask_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Capture the actual current finite Session selection as a persistent source-local layer mask. Empty selection is zero coverage; missing selection never means a numeric payload. Explicit enabled Boolean is required. Atomic and undoable."
    )]
    async fn layer_mask_from_selection(
        &self,
        Parameters(p): Parameters<Enabled>,
    ) -> CallToolResult {
        self.work_unscoped(move |state| {
            let layer = state.layer(p.layer_id)?;
            state.apply(vec![
                json!({"op":"layer.mask.from_selection","layer_ref":layer,"enabled":p.enabled}),
            ])
        })
        .await
    }
    #[tool(
        description = "Enable or disable a stored persistent layer mask without discarding its finite coverage. Does not invent a mask when none is stored."
    )]
    async fn layer_mask_set_enabled(&self, Parameters(p): Parameters<Enabled>) -> CallToolResult {
        self.work_unscoped(move |state| {
            let layer = state.layer(p.layer_id)?;
            state.apply(vec![
                json!({"op":"layer.mask.set_enabled","layer_ref":layer,"enabled":p.enabled}),
            ])
        })
        .await
    }
    #[tool(
        description = "Invert a stored layer mask inside its finite domain, preserving zero outside that domain. Atomic and undoable; no raw A8 payload is accepted."
    )]
    async fn layer_mask_invert(&self, Parameters(p): Parameters<Layer>) -> CallToolResult {
        self.work_unscoped(move |state| {
            let layer = state.layer(p.layer_id)?;
            state.apply(vec![json!({"op":"layer.mask.invert","layer_ref":layer})])
        })
        .await
    }
    #[tool(
        description = "Remove stored layer-mask intent. Removing the last mask restores the minimal result/persistence profile; Undo retains the original coverage."
    )]
    async fn layer_mask_remove(&self, Parameters(p): Parameters<Layer>) -> CallToolResult {
        self.work_unscoped(move |state| {
            let layer = state.layer(p.layer_id)?;
            state.apply(vec![json!({"op":"layer.mask.remove","layer_ref":layer})])
        })
        .await
    }
    #[tool(
        description = "Resolve immutable layer-appearance schemas for four selection-derived layer-mask operations, finite v5 inspection and unchanged thirteen numerical filter IDs. Schema discovery grants no worker or foreign-bake authority."
    )]
    async fn get_layer_appearance_schema(
        &self,
        Parameters(p): Parameters<Schema>,
    ) -> CallToolResult {
        let text = super::super::layer_appearance_schemas::schema(p.kind.name())
            .expect("all layer appearance schema enum variants are embedded");
        CallToolResult::success(vec![ContentBlock::text(text)])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_controls_require_boolean_and_reject_numeric_mask_payloads() {
        assert!(serde_json::from_value::<Enabled>(json!({})).is_err());
        assert!(serde_json::from_value::<Enabled>(json!({"enabled":0})).is_err());
        assert!(serde_json::from_value::<Enabled>(json!({"enabled":true,"mask":{}})).is_err());
        assert!(serde_json::from_value::<Layer>(json!({"coverage":[255]})).is_err());
        assert!(serde_json::from_value::<Enabled>(json!({"enabled":false})).is_ok());
    }
}
