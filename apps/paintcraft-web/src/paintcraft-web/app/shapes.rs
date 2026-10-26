//! Browser shape assets never publish a document or acknowledge project saves.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    csh,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{shape_assets as widget, t};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc, sync::Arc};
mod placement;

#[derive(Debug)]
struct Loaded {
    id: uuid::Uuid,
    library: Arc<csh::Library>,
    view: widget::Resource,
}
impl Loaded {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn new(bytes: &[u8], name: String) -> Result<Self> {
        let mut request = Request::new(
            registry::Frontend::Studio,
            Operation::ImportAsset,
            Family::Csh,
            Platform::Web,
        );
        request.extension = Some(name.rsplit('.').next().unwrap_or(""));
        registry::admission(request).require_allowed(request)?;
        if name.len() > 4096 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "CSH filename exceeds allowance",
            ));
        }
        let library = Arc::new(csh::decode(bytes, &|| Ok(()))?);
        let view = widget::Resource {
            name,
            shapes: library.shapes().iter().map(shape_view).collect(),
        };
        Ok(Self {
            library,
            view,
            id: paintcraft_core::types::ids::deterministic_id(&[
                "shape-library",
                "csh",
                &Sha256::digest(bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            ]),
        })
    }
}
#[derive(Debug, Default)]
struct Inbox {
    epoch: uuid::Uuid,
    result: Option<Result<Option<Loaded>>>,
}
#[derive(Debug, Default)]
pub(super) struct State {
    pub(super) open: bool,
    epoch: uuid::Uuid,
    loaded: Option<Arc<Loaded>>,
    placement: Option<placement::Draft>,
    busy: bool,
    error: Option<String>,
    selected: usize,
    inbox: Rc<RefCell<Inbox>>,
}
impl State {
    fn invalidate(&mut self) {
        self.placement = None;
        self.epoch = uuid::Uuid::new_v4();
        self.busy = false;
        *self.inbox.borrow_mut() = Inbox {
            epoch: self.epoch,
            result: None,
        };
    }
}
impl WebApp {
    pub(super) fn begin_shapes(&mut self) {
        if self.frontend != Frontend::Studio {
            return;
        }
        self.shapes.invalidate();
        self.shapes.open = true;
        self.shapes.error = None;
    }
    fn accept_shape(
        &mut self,
        ctx: &egui::Context,
        epoch: uuid::Uuid,
        result: Result<Option<Loaded>>,
    ) {
        if !self.shapes.open || self.frontend != Frontend::Studio || self.shapes.epoch != epoch {
            return;
        }
        self.shapes.busy = false;
        match result {
            Ok(Some(loaded)) => {
                self.shapes.loaded = Some(Arc::new(loaded));
                self.shapes.selected = 0;
                self.shapes.error = None;
                self.message = t!(ctx, "Shape imported.");
            }
            Ok(None) => {
                self.message = t!(ctx, "Shape import cancelled.");
            }
            Err(error) => {
                self.shapes.error = Some(t!(
                    ctx,
                    "Could not import the shape: { $detail }",
                    detail = error
                ));
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn pick_shape(&mut self, ctx: &egui::Context) {
        if self.shapes.busy {
            return;
        }
        self.shapes.invalidate();
        self.shapes.busy = true;
        self.shapes.error = None;
        let title = t!(ctx, "Import shapes");
        let filter = t!(ctx, "Adobe custom shapes");
        let epoch = self.shapes.epoch;
        let inbox = self.shapes.inbox.clone();
        let repaint = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title(title)
                    .add_filter(filter, &["csh"])
                    .pick_file()
                    .await
                else {
                    return Ok(None);
                };
                if file.inner().size() > csh::BYTES_MAX as f64 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "CSH source exceeds byte allowance",
                    ));
                }
                let buffer = wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer())
                    .await
                    .map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Browser shape read failed")
                    })?;
                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                Ok(Some(Loaded::new(&bytes, file.file_name())?))
            }
            .await;
            if inbox.borrow().epoch == epoch {
                inbox.borrow_mut().result = Some(result);
            }
            repaint.request_repaint();
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn pick_shape(&mut self, ctx: &egui::Context) {
        self.shapes.error = Some(t!(
            ctx,
            "Could not import the shape: { $detail }",
            detail = "File picking is available in the browser build"
        ));
    }
    fn export_shape(&mut self, ctx: &egui::Context) {
        let result: Result<()> = (|| {
            let source = self.shapes.loaded.as_ref().ok_or_else(|| {
                Error::new(ErrorCode::InputSchema, "import a shape before exporting")
            })?;
            let request = Request::new(
                registry::Frontend::Studio,
                Operation::ExportAsset,
                Family::Csh,
                Platform::Web,
            );
            let decision = registry::admission(request);
            if !decision.available() {
                decision.require_allowed(request)?;
            }
            let bytes = csh::encode(&source.library, &|| Ok(()))?;
            let stem = std::path::Path::new(&source.view.name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            download(&bytes, &format!("{stem}-copy.csh"))?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.shapes.error = None;
                self.message = t!(ctx, "Shape download requested.");
            }
            Err(error) => {
                self.shapes.error = Some(t!(
                    ctx,
                    "Could not export the shape: { $detail }",
                    detail = error
                ))
            }
        }
    }
    pub(super) fn shape_dialog(&mut self, ctx: &egui::Context) {
        if !self.shapes.open {
            return;
        }
        if self.frontend != Frontend::Studio {
            self.shapes.open = false;
            self.shapes.invalidate();
            return;
        }
        let result = {
            let mut inbox = self.shapes.inbox.borrow_mut();
            if inbox.epoch == self.shapes.epoch {
                inbox.result.take()
            } else {
                None
            }
        };
        if let Some(result) = result {
            self.accept_shape(ctx, self.shapes.epoch, result);
        }
        if self.shapes.placement.is_some() {
            self.shape_placement_dialog(ctx);
            return;
        }
        match widget::dialog(
            ctx,
            self.shapes.loaded.as_ref().map(|source| &source.view),
            self.shapes.busy,
            self.shapes.error.as_deref(),
            self.shapes.selected,
        ) {
            widget::Response::UseTool => {
                if let Err(error) = self.begin_shape_tool() {
                    self.shapes.error = Some(t!(
                        ctx,
                        "Could not place the shape: { $detail }",
                        detail = error
                    ));
                }
            }
            widget::Response::Place => {
                if let Err(error) = self.begin_shape_placement() {
                    self.shapes.error = Some(t!(
                        ctx,
                        "Could not place the shape: { $detail }",
                        detail = error
                    ));
                }
            }
            widget::Response::Pending => {}
            widget::Response::Close => {
                self.shapes.open = false;
                self.shapes.invalidate();
            }
            widget::Response::Import => self.pick_shape(ctx),
            widget::Response::Export => self.export_shape(ctx),
            widget::Response::Select(index) => self.shapes.selected = index,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn download(bytes: &[u8], name: &str) -> Result<()> {
    crate::files::download_copy(bytes, name, "application/octet-stream")
        .map_err(|_| Error::new(ErrorCode::StorageFailed, "Browser shape download failed"))
}
#[cfg(not(target_arch = "wasm32"))]
fn download(_bytes: &[u8], _name: &str) -> Result<()> {
    Err(Error::new(
        ErrorCode::CapabilityUnsupported,
        "Downloads are available in the browser build",
    ))
}

fn shape_view(shape: &csh::Shape) -> widget::Shape {
    let mut paths: Vec<widget::Path> = Vec::new();
    for record in &shape.records {
        match record {
            csh::Record::Subpath { open, .. } => paths.push(widget::Path {
                open: *open,
                knots: Vec::new(),
            }),
            csh::Record::Knot { points, .. } => {
                if let Some(path) = paths.last_mut() {
                    path.knots.push(points.map(|v| f64::from(v) / 16_777_216.0));
                }
            }
            _ => {}
        }
    }
    widget::Shape {
        name: shape.name.clone(),
        id: shape.id.clone(),
        width: shape.bounds[3] - shape.bounds[1],
        height: shape.bounds[2] - shape.bounds[0],
        paths,
        placement_restrictions: placement_restrictions(shape),
    }
}

#[cfg(test)]
mod tests;

fn placement_restrictions(shape: &csh::Shape) -> Vec<&'static str> {
    match csh::placement::assess(shape, &|| Ok(())) {
        Err(_) => vec!["This shape has unsupported path geometry."],
        Ok(assessment) => assessment
            .unsupported()
            .iter()
            .map(|reason| match reason {
                csh::placement::Unsupported::MultiplePaths => {
                    "Shapes with multiple paths cannot be placed yet."
                }
                csh::placement::Unsupported::OpenPath => "Open paths cannot be placed yet.",
                csh::placement::Unsupported::BooleanOperation => {
                    "This shape uses unsupported Boolean operations."
                }
                csh::placement::Unsupported::InitialFill => {
                    "Shapes that fill the outside area cannot be placed yet."
                }
                csh::placement::Unsupported::Clipboard => {
                    "This shape includes unsupported placement metadata."
                }
                csh::placement::Unsupported::CoordinateRange => {
                    "This shape's control points exceed the placement limits."
                }
                _ => "This shape has unsupported path geometry.",
            })
            .collect(),
    }
}
