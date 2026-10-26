//! Registry-controlled fresh layered export; never a preserving save.
use super::super::{
    PathBuf,
    export_layered::{self, Background, Format, Geometry},
    same_file,
};
use super::{
    PaintingServer,
    state::{core_json, invalid},
};
use paintcraft_formats::registry::Frontend;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Export {
    path: String,
    format: Format,
    /// Required for PDN; ordinary maps native normal layers without adding behavior.
    pdn_background: Option<Background>,
    /// Exact by default; materialize explicitly places/pads each layer with loss consent.
    pdn_geometry: Option<Geometry>,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    allow_losses: Vec<String>,
}
#[tool_router(router=layered_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Export a fresh PSD/PSB/PDN layered copy, only when its registry writer qualification is available. Does not flatten or clear dirty state. Preserving Save is unavailable. Currently refused pending required original-application qualification; loss consent cannot override that gate."
    )]
    async fn export_layered(&self, Parameters(p): Parameters<Export>) -> CallToolResult {
        self.work(move |state| {
            let path = PathBuf::from(p.path);
            export_layered::admit(Frontend::Studio, p.format, &path)?;
            let target = p.format.target(p.pdn_background, p.pdn_geometry)?;
            let protect = || {
                state.protect_copy_sources(&path)?;
                if state
                    .source
                    .as_ref()
                    .is_some_and(|(source, _)| same_file(source, &path))
                {
                    return Err(invalid("layered copy must not replace its source project"));
                }
                Ok(())
            };
            protect()?;
            let doc = state
                .session
                .as_ref()
                .ok_or_else(|| invalid("create or open a document first"))?
                .document();
            let (receipt, ok, _, preparation) = export_layered::write_copy(
                doc,
                target,
                &path,
                p.overwrite,
                &p.allow_losses,
                None,
                &protect,
            )?;
            let mut content = vec![ContentBlock::text(core_json(&receipt)?.to_string())];
            if let Some(preparation) = preparation {
                content.push(ContentBlock::text(core_json(&preparation)?.to_string()));
            }
            Ok(if ok {
                CallToolResult::success(content)
            } else {
                CallToolResult::error(content)
            })
        })
        .await
    }
}
