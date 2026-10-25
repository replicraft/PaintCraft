use super::super::{
    BATCH_V2_SCHEMA, Context, INPUT_MAX, ImageFormat, POLICY, PathBuf, Rgba, Session, export,
    image, new_document, pcnd, read_bounded, save, write_output,
};
use super::{
    PaintingServer,
    params::*,
    state::{core_json, invalid},
};
use base64::Engine as _;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use serde_json::json;

#[tool_router]
impl PaintingServer {
    #[tool(
        description = "Create a canvas with a paintable background layer. Explicit discard_changes is required to replace unsaved work."
    )]
    async fn create_canvas(&self, Parameters(p): Parameters<Create>) -> CallToolResult {
        self.work(move |state| {
            state.replace_allowed(p.discard_changes)?;
            let color = Rgba::parse(p.background.as_deref().unwrap_or("#FFFFFFFF"))
                .ok_or_else(|| invalid("background must be #RRGGBBAA"))?;
            let mut session =
                Session::new(new_document(p.width, p.height, Some(color), 0, POLICY)?);
            session.mark_unsaved();
            state.session = Some(session);
            state.source = None;
            state.inspect()
        })
        .await
    }
    #[tool(
        description = "Open a local .pcnd project, preserving layers. Does not control a desktop window."
    )]
    async fn open_project(&self, Parameters(p): Parameters<Open>) -> CallToolResult {
        self.work(move |state| {
            state.replace_allowed(p.discard_changes)?;
            let path = PathBuf::from(p.path);
            let (bytes, sha) = read_bounded(&path, INPUT_MAX, "project")?;
            let doc = pcnd::read(&bytes)?;
            state.session = Some(Session::new(doc));
            state.source = Some((path, sha));
            state.inspect()
        })
        .await
    }
    #[tool(
        description = "Inspect canvas dimensions, layer IDs in bottom-to-top order, dirty state and undo history."
    )]
    async fn inspect_document(&self) -> CallToolResult {
        self.work(|s| s.inspect()).await
    }

    #[tool(
        description = "Paint a continuous brush stroke. Points use canvas pixel coordinates; color is #RRGGBBAA. Defaults to smooth-round on the top layer."
    )]
    async fn paint_stroke(&self, Parameters(p): Parameters<Stroke>) -> CallToolResult {
        self.work(move |s| {
            let layer = s.layer(p.layer_id)?;
            s.apply(vec![json!({"op":"raster.stroke_path","layer_ref":layer,"points":p.points,
                "color":p.color,"size":p.size,"brush":p.brush.unwrap_or_else(||"smooth-round".into()),"mode":"source-over"})])
        }).await
    }
    #[tool(
        description = "Draw a shape with optional fill and outline. Supply at least one of fill or stroke_color. Colors are #RRGGBBAA."
    )]
    async fn draw_shape(&self, Parameters(p): Parameters<Shape>) -> CallToolResult {
        self.work(move |s| {
            let layer = s.layer(p.layer_id)?;
            let mut op = json!({"op":"raster.shape","layer_ref":layer,"kind":p.kind,"from":p.from,"to":p.to,"mode":"source-over"});
            if let Some(fill) = p.fill {op["fill"] = json!(fill);}
            if let Some(color) = p.stroke_color {op["stroke"] = json!({"color":color,"size":p.stroke_size.unwrap_or(1)});}
            s.apply(vec![op])
        }).await
    }
    #[tool(
        description = "Get the engine batch schema, including all brushes, shapes, layer operations, flood fill, text, selections and transforms."
    )]
    async fn get_batch_schema(&self) -> CallToolResult {
        CallToolResult::success(vec![ContentBlock::text(BATCH_V2_SCHEMA)])
    }
    #[tool(
        description = "Apply engine operations as one atomic, undoable batch. Use get_batch_schema for operation definitions. The server captures the current base ticket; failures publish no edits."
    )]
    async fn apply_operations(&self, Parameters(p): Parameters<Batch>) -> CallToolResult {
        self.work(move |s| s.apply(p.operations)).await
    }
    #[tool(description = "Undo the previous batch, or redo it when redo is true.")]
    async fn history(&self, Parameters(p): Parameters<Navigate>) -> CallToolResult {
        self.work(move |s| {
            if p.redo {
                s.session()?.redo()?;
            } else {
                s.session()?.undo()?;
            }
            s.inspect()
        })
        .await
    }
    #[tool(
        description = "Return a PNG preview of the composited canvas, at most 256 pixels per side. Does not save or alter the project."
    )]
    async fn preview(&self) -> CallToolResult {
        self.work(|s| {
            let frame = paintcraft_render::thumbnail::render_document(
                s.session()?.document(),
                256,
                || Ok(()),
            )?;
            let [w, h] = frame.request.size();
            let bytes = image::encode(ImageFormat::Png, w, h, &frame.pixels, 90)?;
            Ok(CallToolResult::success(vec![ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(bytes),
                "image/png",
            )]))
        })
        .await
    }
    #[tool(
        description = "Save a layered .pcnd project. Existing destinations require overwrite; saving back to the opened project checks for external changes. Successful saves clear dirty state."
    )]
    async fn save_project(&self, Parameters(p): Parameters<Save>) -> CallToolResult {
        self.work(move |s| s.save(PathBuf::from(p.path), p.overwrite))
            .await
    }
    #[tool(
        description = "Export a flattened image selected by filename extension. Requires explicit allow_losses acknowledgements when information would be lost. Does not clear project dirty state."
    )]
    async fn export_image(&self, Parameters(p): Parameters<Export>) -> CallToolResult {
        self.work(move |s| {
            let path = PathBuf::from(p.path);
            let format = ImageFormat::from_extension(&path)
                .ok_or_else(|| invalid("unsupported image extension"))?;
            save::preflight(&path, p.overwrite)?;
            let doc = s.session()?.document();
            let flat = export::export_flat(doc, format, 90, &p.allow_losses)?;
            let ctx = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: None,
                losses_acknowledged: p.allow_losses,
                destination: &path,
            };
            let (receipt, ok, _) = write_output(&path, p.overwrite, Ok(flat.bytes), &ctx);
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

use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool_handler};
#[tool_handler(router = self.tool_router)]
impl ServerHandler for PaintingServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("paintcraft", env!("CARGO_PKG_VERSION")))
            .with_instructions("Headless PaintCraft session, separate from desktop windows. Create or open a project first. Coordinates are canvas pixels; colors are #RRGGBBAA. Edits are atomic and undoable. Save explicitly before disconnecting. File paths are local to the server. Never retry a mutation automatically after losing its response.")
    }
}

impl PaintingServer {
    pub(super) fn router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}
