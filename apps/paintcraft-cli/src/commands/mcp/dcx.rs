//! One-page DCX copies through the same guarded CLI preparation/publication path.
use super::super::{
    PathBuf,
    export_dcx::{self, Mode},
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
    /// Explicit RGB24, exact indexed, or requested quantization policy.
    pcx_mode: Mode,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    allow_losses: Vec<String>,
}
#[tool_router(router=dcx_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Export one DCX page from the document composite using an explicit PCX pixel mode. Requires actual flatten, hidden RGB, alpha, or quantization loss consent. Does not map layers to pages or change the session, saved path, or dirty state. Native Studio fresh copies only; source-preserving Save is unavailable."
    )]
    async fn export_dcx(
        &self,
        Parameters(p): Parameters<Export>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = super::native_codec::RpcLife::new();
        let dropped = life.0.clone();
        let cancellation = context.ct.clone();
        self.work(move |state| {
            let check = || {
                if cancellation.is_cancelled() || dropped.load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::OperationCancelled,
                        "DCX export request was cancelled",
                    ));
                }
                Ok(())
            };
            check()?;
            let path = PathBuf::from(p.path);
            export_dcx::admit(Frontend::Studio, &path)?;
            let protect = || state.protect_copy_sources(&path);
            protect()?;
            let doc = state
                .session
                .as_ref()
                .ok_or_else(|| invalid("create or open a document first"))?
                .document();
            let (receipt, ok, _) = export_dcx::write_copy(
                doc,
                p.pcx_mode,
                &path,
                p.overwrite,
                &p.allow_losses,
                None,
                &protect,
                &check,
            )?;
            let content = vec![ContentBlock::text(core_json(&receipt)?.to_string())];
            Ok(if ok {
                CallToolResult::success(content)
            } else {
                CallToolResult::error(content)
            })
        })
        .await
    }
}
