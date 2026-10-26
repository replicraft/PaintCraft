//! Browser palette assets never publish a document or acknowledge project saves.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    ase,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{palette as widget, t};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
struct Loaded {
    palette: ase::Palette,
    view: widget::Resource,
}
impl Loaded {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn new(bytes: &[u8], name: String) -> Result<Self> {
        let mut request = Request::new(
            registry::Frontend::Studio,
            Operation::ImportAsset,
            Family::Ase,
            Platform::Web,
        );
        request.extension = Some(name.rsplit('.').next().unwrap_or(""));
        registry::admission(request).require_allowed(request)?;
        let palette = ase::decode(bytes, &|| Ok(()))?;
        let mut rows = Vec::new();
        let swatch = |color: &ase::Swatch| {
            widget::Row::Swatch(widget::Swatch {
                name: color.name.clone(),
                model: color.color.model(),
                kind: match color.kind {
                    ase::Kind::Global => "Global color",
                    ase::Kind::Spot => "Spot color",
                    ase::Kind::Normal => "Process color",
                },
                components: color
                    .color
                    .channels()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                rgba: color.color.srgba8(),
            })
        };
        for entry in palette.entries() {
            match entry {
                ase::Entry::Swatch(color) => rows.push(swatch(color)),
                ase::Entry::Group { name, swatches } => {
                    rows.push(widget::Row::Group(name.clone()));
                    rows.extend(swatches.iter().map(swatch));
                }
            }
        }
        Ok(Self {
            palette,
            view: widget::Resource { name, rows },
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
    loaded: Option<Loaded>,
    busy: bool,
    error: Option<String>,
    inbox: Rc<RefCell<Inbox>>,
}
impl State {
    fn invalidate(&mut self) {
        self.epoch = uuid::Uuid::new_v4();
        self.busy = false;
        *self.inbox.borrow_mut() = Inbox {
            epoch: self.epoch,
            result: None,
        };
    }
}
impl WebApp {
    pub(super) fn begin_palettes(&mut self) {
        if self.frontend != Frontend::Studio {
            return;
        }
        self.palettes.invalidate();
        self.palettes.open = true;
        self.palettes.error = None;
    }
    fn accept_palette(
        &mut self,
        ctx: &egui::Context,
        epoch: uuid::Uuid,
        result: Result<Option<Loaded>>,
    ) {
        if !self.palettes.open || self.frontend != Frontend::Studio || self.palettes.epoch != epoch
        {
            return;
        }
        self.palettes.busy = false;
        match result {
            Ok(Some(loaded)) => {
                self.palettes.loaded = Some(loaded);
                self.palettes.error = None;
                self.message = t!(ctx, "Palette imported.");
            }
            Ok(None) => {
                self.message = t!(ctx, "Palette import cancelled.");
            }
            Err(error) => {
                self.palettes.error = Some(t!(
                    ctx,
                    "Could not import the palette: { $detail }",
                    detail = error
                ));
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn pick_palette(&mut self, ctx: &egui::Context) {
        if self.palettes.busy {
            return;
        }
        self.palettes.invalidate();
        self.palettes.busy = true;
        self.palettes.error = None;
        let title = t!(ctx, "Import palette");
        let filter = t!(ctx, "Adobe Swatch Exchange palette");
        let epoch = self.palettes.epoch;
        let inbox = self.palettes.inbox.clone();
        let repaint = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title(title)
                    .add_filter(filter, &["ase"])
                    .pick_file()
                    .await
                else {
                    return Ok(None);
                };
                if file.inner().size() > ase::BYTES_MAX as f64 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "ASE source exceeds byte allowance",
                    ));
                }
                let buffer = wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer())
                    .await
                    .map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Browser palette read failed")
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
    fn pick_palette(&mut self, ctx: &egui::Context) {
        self.palettes.error = Some(t!(
            ctx,
            "Could not import the palette: { $detail }",
            detail = "File picking is available in the browser build"
        ));
    }
    fn export_palette(&mut self, ctx: &egui::Context) {
        let result: Result<()> = (|| {
            let source = self.palettes.loaded.as_ref().ok_or_else(|| {
                Error::new(ErrorCode::InputSchema, "import a palette before exporting")
            })?;
            let request = Request::new(
                registry::Frontend::Studio,
                Operation::ExportAsset,
                Family::Ase,
                Platform::Web,
            );
            let decision = registry::admission(request);
            if !decision.available() {
                decision.require_allowed(request)?;
            }
            let bytes = ase::encode(&source.palette, &|| Ok(()))?;
            let stem = std::path::Path::new(&source.view.name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            download(&bytes, &format!("{stem}-copy.ase"))?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.palettes.error = None;
                self.message = t!(ctx, "Palette download requested.");
            }
            Err(error) => {
                self.palettes.error = Some(t!(
                    ctx,
                    "Could not export the palette: { $detail }",
                    detail = error
                ))
            }
        }
    }
    pub(super) fn palette_dialog(&mut self, ctx: &egui::Context) {
        if !self.palettes.open {
            return;
        }
        if self.frontend != Frontend::Studio {
            self.palettes.open = false;
            self.palettes.invalidate();
            return;
        }
        let result = {
            let mut inbox = self.palettes.inbox.borrow_mut();
            if inbox.epoch == self.palettes.epoch {
                inbox.result.take()
            } else {
                None
            }
        };
        if let Some(result) = result {
            self.accept_palette(ctx, self.palettes.epoch, result);
        }
        match widget::dialog(
            ctx,
            self.palettes.loaded.as_ref().map(|source| &source.view),
            self.palettes.busy,
            self.palettes.error.as_deref(),
        ) {
            widget::Response::Pending => {}
            widget::Response::Close => {
                self.palettes.open = false;
                self.palettes.invalidate();
            }
            widget::Response::Import => self.pick_palette(ctx),
            widget::Response::Export => self.export_palette(ctx),
            widget::Response::Pick(rgba) => {
                if self.studio.edit_secondary {
                    self.controls.secondary = paintcraft_core::types::Rgba(rgba);
                } else {
                    self.controls.primary = paintcraft_core::types::Rgba(rgba);
                }
                self.palettes.open = false;
                self.palettes.invalidate();
            }
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
fn download(bytes: &[u8], name: &str) -> Result<()> {
    crate::files::download_copy(bytes, name, "application/octet-stream")
        .map_err(|_| Error::new(ErrorCode::StorageFailed, "Browser palette download failed"))
}
#[cfg(not(target_arch = "wasm32"))]
fn download(_bytes: &[u8], _name: &str) -> Result<()> {
    Err(Error::new(
        ErrorCode::CapabilityUnsupported,
        "Downloads are available in the browser build",
    ))
}
