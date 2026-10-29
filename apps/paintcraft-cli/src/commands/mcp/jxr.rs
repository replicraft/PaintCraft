//! Native JXR copies reuse shared preparation and coordinated staged publication.
use super::super::{PathBuf, codec_worker, export_jxr};
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
    /// Explicitly omit original source metadata from this copy; default refuse.
    source_metadata: Option<super::params::SourceMetadata>,
    path: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    allow_losses: Vec<String>,
}
#[tool_router(router=jxr_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Export a native Studio fresh JPEG XR copy using fixed lossless opaque RGB8. Requires actual flattening/metadata loss consent, refuses transparent composites, and keeps native saved path, document and dirty state unchanged."
    )]
    async fn export_jxr(
        &self,
        Parameters(p): Parameters<Export>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = super::native_codec::RpcLife::new();
        let dropped = life.0.clone();
        let cancellation = context.ct.clone();
        self.work(move |state| {
            let check: super::super::jxr_export_job::Check = std::sync::Arc::new(move || {
                if cancellation.is_cancelled() || dropped.load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::OperationCancelled,
                        "JXR export request was cancelled",
                    ));
                }
                Ok(())
            });
            check()?;
            let path = PathBuf::from(p.path);
            export_jxr::admit(Frontend::Studio, &path)?;
            let protect = |destination: &std::path::Path| state.protect_copy_sources(destination);
            protect(&path)?;
            let doc = state
                .session
                .as_ref()
                .ok_or_else(|| invalid("create or open a document first"))?
                .document();
            let empty = codec_worker::owner();
            let captured = super::native_codec::owner(state, empty);
            let current = || Ok(super::native_codec::owner(state, empty));
            let intent: paintcraft_formats::export::metadata_policy::Intent =
                p.source_metadata.unwrap_or_default().into();
            let policy = super::super::export_metadata::CopyFormat::Jxr.capture(doc, intent)?;
            let (receipt, ok, _, details) =
                if intent == paintcraft_formats::export::metadata_policy::Intent::Refuse {
                    export_jxr::write_copy(
                        doc,
                        &path,
                        p.overwrite,
                        &p.allow_losses,
                        None,
                        captured,
                        &current,
                        super::native_codec::preparation_budget(),
                        &protect,
                        check,
                    )?
                } else {
                    export_jxr::write_copy_with_metadata_policy(
                        doc,
                        &path,
                        p.overwrite,
                        &p.allow_losses,
                        None,
                        captured,
                        &current,
                        super::native_codec::preparation_budget(),
                        &protect,
                        check,
                        policy,
                    )?
                };

            let value = paintcraft_core::types::json::Value::object([
                ("export", details),
                ("save", receipt),
            ]);
            let content = vec![ContentBlock::text(core_json(&value)?.to_string())];
            Ok(if ok {
                CallToolResult::success(content)
            } else {
                CallToolResult::error(content)
            })
        })
        .await
    }
}
