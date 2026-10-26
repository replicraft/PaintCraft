//! Shape resources are independent of the current painting document/session.
use super::super::{PathBuf, same_file, shapes};
use super::{
    PaintingServer,
    state::{core_json, invalid, reply},
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
struct Inspect {
    /// Local .csh shape library resource, not a project or image.
    path: String,
    /// Optional exact lowercase SHA-256 of the acquired source bytes.
    expect_input_sha256: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Export {
    /// Local source .csh shape library. Exact integer control records are retained.
    input: String,
    /// Fresh .csh destination, distinct from every protected source.
    path: String,
    expect_input_sha256: Option<String>,
    #[serde(default)]
    overwrite: bool,
}
#[tool_router(router = shapes_router, vis = "pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Inspect a Photoshop custom shape (.csh) library as an asset without creating or changing a painting. Returns the shape-inspection/v1 contract: ordered shape names, original IDs and bounds, ordered path/fill/clipboard records, exact signed normalized 8.24 i32 control coordinates and every operation/link/open/fill flag. Does not require a document."
    )]
    async fn inspect_shapes(&self, Parameters(p): Parameters<Inspect>) -> CallToolResult {
        self.work(move |_| {
            let (resource, sha) = shapes::acquire(
                &PathBuf::from(p.path),
                p.expect_input_sha256.as_deref(),
                Frontend::Studio,
            )?;
            Ok(reply(core_json(&shapes::inspection(&resource, &sha)?)?))
        })
        .await
    }
    #[tool(
        description = "Export a fresh .csh copy of a local shape library. Retains names, original bounds, ordered records, exact i32 control coordinates and all operation/fill flags. Does not change document pixels, history, dirty state or save binding; works without a painting. Never overwrites its source or the current painting's source resources."
    )]
    async fn export_shapes(&self, Parameters(p): Parameters<Export>) -> CallToolResult {
        self.work(move |state| {
            let path = PathBuf::from(p.path);
            state.protect_import_sources(&path)?;
            if state
                .source
                .as_ref()
                .is_some_and(|(source, _)| same_file(source, &path))
            {
                return Err(invalid(
                    "shapes export must not replace the open project source",
                ));
            }
            let written = shapes::export_copy(
                &PathBuf::from(p.input),
                &path,
                p.overwrite,
                p.expect_input_sha256.as_deref(),
                Frontend::Studio,
            )?;
            let result = crate::report::envelope(
                written.engine,
                Some(written.receipt),
                if written.ok {
                    "success"
                } else {
                    "file-incomplete"
                },
                if written.ok { 0 } else { 6 },
            );
            let content = vec![ContentBlock::text(core_json(&result)?.to_string())];
            Ok(if written.ok {
                CallToolResult::success(content)
            } else {
                CallToolResult::error(content)
            })
        })
        .await
    }
}
