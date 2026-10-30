use super::super::format_profile;
use super::super::{
    BATCH_V2_SCHEMA, Context, INPUT_MAX, ImageFormat, POLICY, PathBuf, Rgba, Session, export,
    image, new_document, read_captured, read_conversion_source, save,
};
use super::{
    PaintingServer,
    params::*,
    state::{core_json, invalid},
};
use base64::Engine as _;
use paintcraft_formats::registry::Family;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use serde_json::json;
use sha2::{Digest, Sha256};

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
            state.project_identity = None;
            state.import_source = None;
            state.import_identity = None;
            state.import_resources.clear();
            state.brush_sources.clear();
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
            let (bytes, sha, identity) = read_captured(&path, INPUT_MAX, "project")?;
            if format_profile::Options::default().admit_open(&path, &bytes)? != Family::Pcnd {
                return Err(invalid(
                    "open_project expects PCND; use import_image for pictures",
                ));
            }
            let doc = state.metadata_realm.read(&bytes, &|| Ok(()))?;
            state.session = Some(Session::new(doc));
            state.source = Some((path, sha));
            state.project_identity = Some(identity);
            state.import_source = None;
            state.import_identity = None;
            state.import_resources.clear();
            state.brush_sources.clear();
            state.inspect()
        })
        .await
    }
    #[tool(
        description = "Import a local picture into a new unsaved project using the CLI Studio format profile. Missing loss acknowledgements or decoding failures preserve the current document. Use allow_losses to explicitly accept reported conversion losses."
    )]
    async fn import_image(
        &self,
        Parameters(p): Parameters<Import>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = super::native_codec::RpcLife::new();
        let dropped = life.0.clone();
        let cancellation = context.ct.clone();
        self.work(move |state| {
            state.replace_allowed(p.discard_changes)?;
            if usize::from(p.page.is_some())
                + usize::from(p.entry.is_some())
                + usize::from(p.svg.is_some())
                > 1
            {
                return Err(invalid(
                    "page, entry and svg options are mutually exclusive",
                ));
            }
            if let Some(svg) = &p.svg {
                svg.options()?;
            }
            if let Some(code) = p.allow_losses.iter().find(|code| {
                !image::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::project_import::KNOWN_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::icons::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::icns::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::jxr::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::heif::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::avif::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::avif::sdr::KNOWN_IMPORT_LOSSES.contains(&code.as_str())
                    && !paintcraft_formats::svg::KNOWN_LOSSES.contains(&code.as_str())
                    && code.as_str() != paintcraft_formats::tiff_pages::LOSS_OTHER_PAGES
            }) {
                return Err(invalid(&format!("unknown import loss code {code:?}")));
            }
            let path = PathBuf::from(p.path);
            let native_codec = path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Family::from_extension)
                .is_some_and(|family| matches!(family, Family::Jxl | Family::Jxr | Family::Heif | Family::Avif));
            if native_codec {
                if p.page.is_some() || p.entry.is_some() || p.svg.is_some() {
                    return Err(invalid("Native JXL/JXR/HEIF/AVIF import does not accept page, entry or SVG options"));
                }
                return super::native_codec::import(state, &path,&p.allow_losses, std::sync::Arc::new(move || {
                    if cancellation.is_cancelled() || dropped.load(std::sync::atomic::Ordering::Acquire) {
                        Err(paintcraft_core::types::Error::new(paintcraft_core::types::ErrorCode::OperationCancelled,"MCP codec import cancelled"))
                    } else { Ok(()) }
                }));
            }
            let check = || {
                if cancellation.is_cancelled() || dropped.load(std::sync::atomic::Ordering::Acquire) {
                    Err(paintcraft_core::types::Error::new(paintcraft_core::types::ErrorCode::OperationCancelled, "MCP image import cancelled"))
                } else { Ok(()) }
            };
            check()?;
            let profile = format_profile::Options::default();
            if p.svg.is_some() {
                profile.admit_import_svg_path(&path)?;
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Family::from_extension)
                == Some(Family::Svg)
            {
                return Err(invalid(
                    "SVG import requires explicit width and height in svg options",
                ));
            }
            let captured_limit = path.extension().and_then(|ext|ext.to_str())
                .and_then(Family::from_extension).and_then(|family|match family {
                    Family::Msp=>Some(paintcraft_formats::msp::BYTES_MAX),
                    Family::Icns=>Some(paintcraft_formats::icns::BYTES_MAX),
                    Family::Dds=>Some(paintcraft_formats::dds::BYTES_MAX),
                    Family::Tiff if p.page.is_some()=>Some(paintcraft_formats::tiff_pages::BYTES_MAX),
                    _=>None,
                });
            let (bytes,sha,source_identity) = if let Some(limit)=captured_limit {
                let (bytes,sha,identity)=read_captured(&path,limit as u64,"captured image")?;
                (bytes,sha,Some(identity))
            } else {
                read_conversion_source(
                    &path,
                    if p.svg.is_some() {paintcraft_formats::svg::SOURCE_MAX as u64} else {INPUT_MAX},
                    "image",
                )?
            };
            let raw_sha256: [u8; 32] = Sha256::digest(&bytes).into();
            let family = if p.svg.is_some() {
                profile.admit_import_svg(&path, &bytes)?
            } else {
                match (p.page, p.entry) {
                    (Some(page), _) => profile.admit_import_page(&path, &bytes, page)?,
                    (_, Some(entry)) => profile.admit_import_entry(&path, &bytes, entry)?,
                    _ => profile.admit_import(&path, &bytes)?,
                }
            };
            if matches!(family, Family::Dds | Family::Msp) && source_identity.is_none() {
                return Err(invalid("DDS/MSP import requires a captured regular source"));
            }
            if family == Family::Tiff && p.page.is_some() && source_identity.is_none() {
                return Err(invalid("TIFF page import requires a captured regular source"));
            }
            let mut icns_entry = None;
            let mut tiff_assessment = None;
            let (doc, losses) = if let Some(svg) = &p.svg {
                if !p.allow_losses.iter().any(|code| code == "svg-rasterized") {
                    return Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                        "SVG rasterization requires svg-rasterized in allow_losses",
                    ));
                }
                svg.convert(&bytes, &sha)?
            } else if paintcraft_formats::project_import::supports(family) {
                let converted=if family==Family::Ora && source_identity.is_some() {
                    profile.admit_import_ora(&path,&bytes)?;
                    paintcraft_formats::project_import::decode_ora_with_neutral_groups(&bytes,4_194_304,&check)?
                } else {
                    // Legacy streams use the flat profile upfront; no decoder-error fallback.
                    paintcraft_formats::project_import::decode_layered(family,&bytes,4_194_304,&check)?
                };
                (converted.document, converted.losses)
            } else if family == Family::Icns {
                let entry=p.entry.ok_or_else(||invalid("ICNS import requires an explicit entry"))?;
                let decoded=paintcraft_formats::icns::decode_entry(&bytes,entry,image::DECODE_PIXELS_MAX,&||Ok(()))?;
                let document=decoded.document(&sha)?;
                icns_entry=Some(decoded.entry);
                (document,decoded.losses)
            } else if let Some(page) = p.page.filter(|_|family==Family::Tiff) {
                let converted=super::super::tiff_page::convert(&bytes,page,&sha,&check)?;
                tiff_assessment=Some(core_json(&converted.assessment)?);
                (converted.document,converted.losses)
            } else if let Some(entry) = p.entry {
                let decoded = paintcraft_formats::icons::decode_entry(
                    &bytes,
                    entry,
                    image::DECODE_PIXELS_MAX,
                )?;
                (decoded.document(&sha)?, decoded.losses)
            } else if matches!(family, Family::Png | Family::Bmp) && p.page.is_none() {
                let prepared = if family == Family::Png {
                    state.metadata_realm.png(&bytes, &check)?
                } else {
                    state.metadata_realm.bmp(&bytes, &check)?
                };
                let document = prepared.document_with_admission(
                    &mut paintcraft_raster::Admission::new(paintcraft_core::types::policy::STAGED_PIXELS_MAX),
                    &check)?;
                (document, prepared.image.losses)
            } else {
                profile.reject_project_import(family)?;
                let decoded = if let Some(page) = p.page {
                    paintcraft_formats::dcx::decode_page(&bytes, page, image::DECODE_PIXELS_MAX)?
                } else {
                    image::decode(&bytes)?
                };
                let document_key = if let Some(page)=p.page {
                    paintcraft_document::RasterConversionKey::checked(
                        paintcraft_document::RasterConversionContext {
                            source_sha256:raw_sha256,
                            profile:paintcraft_formats::dcx::IMPORT_IDENTITY_PROFILE,
                            selection:paintcraft_document::RasterSelection::Page(page),assessment_sha256:None},
                        decoded.width,decoded.height,&decoded.rgba,image::DECODE_PIXELS_MAX,&check,
                    )?.hex()
                } else { image::document_conversion_key(&bytes,&decoded,&check)?.map(|key|key.hex()).unwrap_or_else(||sha.clone()) };
                (paintcraft_document::import_image(decoded.width,decoded.height,&decoded.rgba,&document_key)?,decoded.losses)
            };
            let missing = losses
                .iter()
                .filter(|loss| !p.allow_losses.contains(loss))
                .cloned()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                    format!(
                        "import would lose {}; acknowledge with allow_losses",
                        missing.join(", ")
                    ),
                ));
            }
            check()?;
            if let Some(identity) = &source_identity {
                #[cfg(test)]
                stock_source_tests::before_adoption(&path);
                identity.verify_bytes(&path, bytes.len() as u64, &raw_sha256, &check)?;
            }
            let mut session = Session::new(doc);
            session.mark_unsaved();
            state.session = Some(session);
            // A picture is not a native project binding: saving must not replace its source implicitly.
            state.source = None;
            state.project_identity = None;
            state.import_source = Some(path);
            let ora_variant=(family==Family::Ora && source_identity.is_some()).then_some(paintcraft_formats::ora::NEUTRAL_GROUP_IMPORT_VARIANT);
            state.import_identity = source_identity;
            state.brush_sources.clear();
            state.import_resources = p
                .svg
                .map(|selection| selection.font_files)
                .unwrap_or_default();
            if tiff_assessment.is_some() || matches!(family,Family::Msp|Family::Icns|Family::Dds|Family::Ora) {
                let mut value=state.inspection()?;
                value["conversion"]=json!({"source_sha256":sha,"losses_acknowledged":losses,"source_lineage_adopted":false,"source_format":family.id(),"icns_entry":icns_entry.map(|entry|json!({"index":entry.index,"record":entry.record,"tag":String::from_utf8_lossy(&entry.tag),"width":entry.width,"height":entry.height,"logical_width":entry.logical_width,"logical_height":entry.logical_height,"scale":entry.scale})),"preserving_save":false});
                if let Some(variant) = ora_variant {
                    value["conversion"]["profile_variant"] = json!(variant);
                }
                if let Some(assessment)=tiff_assessment {
                    value["conversion"]["profile_variant"]=json!(paintcraft_formats::tiff_pages::IMPORT_VARIANT);
                    value["conversion"]["source_assessment"]=assessment;
                }
                Ok(super::state::reply(value))
            } else {state.inspect()}
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
        description = "Flatten the visible canvas into one layer as a single undoable edit. Hidden layers and off-canvas pixels are removed from the current state; Undo restores them. Does nothing when there is at most one layer."
    )]
    async fn flatten_image(&self) -> CallToolResult {
        self.work(|state| {
            let receipt = state
                .session()?
                .flatten(|doc, check| paintcraft_render::render_cancellable(doc, check))?;
            Ok(super::state::reply(core_json(&receipt.to_value())?))
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
        description = "Export a flattened image selected by filename extension. Retained source metadata requires source_metadata=omit; allow_losses acknowledges other losses. Does not clear project dirty state."
    )]
    async fn export_image(
        &self,
        Parameters(p): Parameters<Export>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = super::native_codec::RpcLife::new();
        let dropped = life.0.clone();
        let cancellation = context.ct.clone();
        self.work(move |s| {
            let path = PathBuf::from(p.path);
            s.protect_copy_sources(&path)?;
            let format = ImageFormat::from_extension(&path)
                .ok_or_else(|| invalid("unsupported image extension"))?;
            let metadata_intent: export::metadata_policy::Intent =
                p.source_metadata.unwrap_or_default().into();
            let jpeg_matte = p
                .jpeg_matte
                .as_deref()
                .map(str::parse::<paintcraft_formats::image::JpegMatte>)
                .transpose()?;
            if jpeg_matte.is_some() && format != ImageFormat::Jpeg {
                return Err(invalid("jpeg_matte requires JPEG output"));
            }
            if p.pcx_encoding.is_some() && format != ImageFormat::Pcx {
                return Err(invalid("pcx_encoding requires PCX output"));
            }
            if let Some(version) = p.msp_version {
                version.require_format(format)?;
                if p.dds.is_some() {
                    return Err(invalid("DDS and MSP options cannot be combined"));
                }
                let check = || {
                    if cancellation.is_cancelled()
                        || dropped.load(std::sync::atomic::Ordering::Acquire)
                    {
                        return Err(paintcraft_core::types::Error::new(
                            paintcraft_core::types::ErrorCode::OperationCancelled,
                            "MSP export request was cancelled",
                        ));
                    }
                    Ok(())
                };
                return if metadata_intent == export::metadata_policy::Intent::Refuse {
                    super::msp::export(s, &path, p.overwrite, version, &p.allow_losses, &check)
                } else {
                    super::msp::export_with_metadata_policy(
                        s,
                        &path,
                        p.overwrite,
                        version,
                        &p.allow_losses,
                        &check,
                        metadata_intent,
                    )
                };
            }
            if let Some(options) = p.dds {
                options.require_format(format)?;
                let check = || {
                    if cancellation.is_cancelled()
                        || dropped.load(std::sync::atomic::Ordering::Acquire)
                    {
                        return Err(paintcraft_core::types::Error::new(
                            paintcraft_core::types::ErrorCode::OperationCancelled,
                            "DDS export request was cancelled",
                        ));
                    }
                    Ok(())
                };
                return if metadata_intent == export::metadata_policy::Intent::Refuse {
                    super::dds::export(s, &path, p.overwrite, options, &p.allow_losses, &check)
                } else {
                    super::dds::export_with_metadata_policy(
                        s,
                        &path,
                        p.overwrite,
                        options,
                        &p.allow_losses,
                        &check,
                        metadata_intent,
                    )
                };
            }
            let family = Family::from_id(format.name())
                .ok_or_else(|| invalid("unregistered image format"))?;
            format_profile::Options::default().admit_export(family, &path)?;
            save::preflight(&path, p.overwrite)?;
            let doc = s.session()?.document();
            let mut options = export::FlatOptions::new(format, 90);
            options.jpeg_matte = jpeg_matte;
            options.source_metadata = export::metadata_policy::Policy::capture(
                doc,
                export::metadata_policy::Target::new(
                    family,
                    paintcraft_formats::registry::Operation::ExportFlat,
                ),
                metadata_intent,
            )?;
            options.pcx_mode = p.pcx_encoding.unwrap_or_default().into();
            let check = || {
                if cancellation.is_cancelled() || dropped.load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::OperationCancelled,
                        "Image export request was cancelled",
                    ));
                }
                Ok(())
            };
            let flat = export::export_flat_options_with_check(
                doc,
                options,
                &p.allow_losses,
                |document| paintcraft_render::render_cancellable(document, check),
                &check,
            )?;
            let ctx = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: None,
                losses_acknowledged: flat.losses,
                destination: &path,
            };
            check()?;
            let outcome = s.write_copy_with_checked(
                &path,
                p.overwrite,
                |sink| {
                    for chunk in flat.bytes.chunks(65_536) {
                        check()?;
                        sink.write_all(chunk)
                            .map_err(|error| super::super::storage("staged image", &error))?;
                    }
                    check()
                },
                &check,
            )?;
            let (receipt, ok, _) = super::super::finish_output(&path, &ctx, outcome);
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
            .with_instructions("Headless PaintCraft session, separate from desktop windows. Painting tools require a project; palette and shape inspection/export work independently. Coordinates are canvas pixels; colors are #RRGGBBAA. Edits are atomic and undoable. Save explicitly before disconnecting. File paths are local to the server. Never retry a mutation automatically after losing its response.")
    }
}

impl PaintingServer {
    pub(super) fn router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
            + Self::icon_router()
            + Self::icns_router()
            + Self::source_inspection_router()
            + Self::dcx_router()
            + Self::jxr_router()
            + Self::float_router()
            + Self::layered_router()
            + Self::palette_router()
            + Self::shapes_router()
            + Self::brush_router()
    }
}

#[cfg(test)]
#[path = "tools/stock_source_tests.rs"]
mod stock_source_tests;
