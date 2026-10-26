//! Browser brush resources use explicit native-size static tip adoption.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, resources::SharedBudget};
use paintcraft_formats::{
    abr::{
        self,
        adoption::{self, RequiredConversion, StaticTipPolicy},
    },
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{brush_resources as widget, t};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc, sync::Arc};
#[derive(Debug)]
struct Loaded {
    id: uuid::Uuid,
    library: Arc<abr::Library>,
    view: widget::Resource,
}
impl Loaded {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn new(bytes: &[u8], name: String) -> Result<Self> {
        if bytes.len() > abr::BYTES_MAX || name.len() > 4096 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Brush source exceeds allowance",
            ));
        }
        let mut request = Request::new(
            registry::Frontend::Studio,
            Operation::ImportAsset,
            Family::Abr,
            Platform::Web,
        );
        request.extension = Some(name.rsplit('.').next().unwrap_or(""));
        registry::admission(request).require_allowed(request)?;
        let library = Arc::new(abr::decode(bytes, &|| Ok(()))?);
        let view = brush_view(&library, name)?;
        let sha = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Self {
            id: paintcraft_core::types::ids::deterministic_id(&["brush-library", "abr", &sha]),
            library,
            view,
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
    owner: Option<(uuid::Uuid, u64)>,
    loaded: Option<Loaded>,
    busy: bool,
    selected: usize,
    controls: widget::State,
    error: Option<String>,
    inbox: Rc<RefCell<Inbox>>,
}
impl WebApp {
    pub(super) fn begin_brushes(&mut self) {
        if self.frontend != Frontend::Studio {
            return;
        }
        self.brushes.epoch = uuid::Uuid::new_v4();
        self.brushes.owner = Some({
            let ticket = self.editor.session.ticket();
            (ticket.state_id, ticket.publication_epoch)
        });
        self.brushes.open = true;
        self.brushes.busy = false;
        self.brushes.controls.reset();
        self.brushes.error = None;
        *self.brushes.inbox.borrow_mut() = Inbox {
            epoch: self.brushes.epoch,
            result: None,
        };
    }
    fn adopt_brush(&mut self) -> Result<()> {
        if self.brushes.owner
            != Some({
                let ticket = self.editor.session.ticket();
                (ticket.state_id, ticket.publication_epoch)
            })
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Painting changed while selecting a brush",
            ));
        }
        let source = self
            .brushes
            .loaded
            .as_ref()
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "No brush resource loaded"))?;
        let ack = if self.brushes.controls.consent {
            &[RequiredConversion::QuantizedSpacing][..]
        } else {
            &[]
        };
        let adopted = adoption::adopt_native(
            source.library.clone(),
            self.brushes.selected,
            StaticTipPolicy::NativeGridV1,
            ack,
            &SharedBudget::new(64 << 20),
            &|| Ok(()),
        )?;
        self.editor.cancel();
        self.editor.tools.sampled_tip = Some((adopted.tip().clone(), adopted.spacing_q8()));
        self.editor.adopted_brush = Some(adopted);
        self.editor.tools.sampled_source = Some(source.id);
        self.editor.tools.sampled_name =
            Some(source.view.presets[self.brushes.selected].name.clone());
        self.controls.tool = Tool::Brush;
        self.editor.tools.choose_tool(Tool::Brush);
        self.brushes.open = false;
        Ok(())
    }
    pub(super) fn brush_dialog(&mut self, ctx: &egui::Context) {
        if self.frontend != Frontend::Studio
            || self.brushes.owner
                != Some({
                    let ticket = self.editor.session.ticket();
                    (ticket.state_id, ticket.publication_epoch)
                })
        {
            self.brushes.open = false;
            self.brushes.epoch = uuid::Uuid::new_v4();
            return;
        }
        let completion = {
            let mut inbox = self.brushes.inbox.borrow_mut();
            if inbox.epoch == self.brushes.epoch {
                inbox.result.take()
            } else {
                None
            }
        };
        if let Some(result) = completion {
            self.brushes.busy = false;
            match result {
                Ok(Some(source)) => {
                    self.brushes.loaded = Some(source);
                    self.brushes.selected = 0;
                    self.brushes.controls.reset();
                    self.brushes.error = None;
                }
                Ok(None) => {}
                Err(error) => {
                    self.brushes.error = Some(t!(
                        ctx,
                        "Could not import brushes: { $detail }",
                        detail = error
                    ))
                }
            }
        }
        let result = match self.brushes.controls.dialog(
            ctx,
            self.brushes.loaded.as_ref().map(|s| &s.view),
            self.brushes.selected,
            self.brushes.busy,
            self.brushes.error.as_deref(),
            self.editor.tools.sampled_name.as_deref(),
        ) {
            widget::Response::Pending => Ok(()),
            widget::Response::Close => {
                self.brushes.open = false;
                Ok(())
            }
            widget::Response::Import => {
                self.pick_brushes(ctx);
                Ok(())
            }
            widget::Response::Select(index) => {
                self.brushes.selected = index;
                self.brushes.controls.reset();
                Ok(())
            }
            widget::Response::Adopt => self.adopt_brush(),
            widget::Response::Stock => {
                self.editor.cancel();
                self.editor.adopted_brush = None;
                self.editor.tools.sampled_tip = None;
                self.editor.tools.sampled_source = None;
                self.editor.tools.sampled_name = None;
                self.brushes.open = false;
                Ok(())
            }
        };
        if let Err(error) = result {
            self.brushes.error = Some(t!(
                ctx,
                "Could not use this brush: { $detail }",
                detail = error
            ));
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn pick_brushes(&mut self, ctx: &egui::Context) {
        if self.brushes.busy {
            return;
        }
        self.brushes.busy = true;
        let epoch = self.brushes.epoch;
        let inbox = self.brushes.inbox.clone();
        let repaint = ctx.clone();
        let title = t!(ctx, "Import brushes");
        let filter = t!(ctx, "Adobe brushes");
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title(title)
                    .add_filter(filter, &["abr"])
                    .pick_file()
                    .await
                else {
                    return Ok(None);
                };
                if file.inner().size() > abr::BYTES_MAX as f64 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "Brush source exceeds allowance",
                    ));
                }
                let buffer = wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer())
                    .await
                    .map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Browser brush read failed")
                    })?;
                Ok(Some(Loaded::new(
                    &js_sys::Uint8Array::new(&buffer).to_vec(),
                    file.file_name(),
                )?))
            }
            .await;
            if inbox.borrow().epoch == epoch {
                inbox.borrow_mut().result = Some(result);
            }
            repaint.request_repaint();
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn pick_brushes(&mut self, ctx: &egui::Context) {
        self.brushes.error = Some(t!(
            ctx,
            "Could not import brushes: { $detail }",
            detail = "Browser picker unavailable"
        ));
    }
}

fn brush_view(library: &abr::Library, name: String) -> Result<widget::Resource> {
    if library.brushes().len() > adoption::PRESETS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Brush preset inventory exceeds allowance",
        ));
    }
    let presets = library
        .brushes()
        .iter()
        .enumerate()
        .map(|(index, brush)| {
            let assessment = adoption::assess(library, index, &|| Ok(()))?;
            Ok(widget::Preset {
                name: brush.name.clone(),
                dimensions: assessment.dimensions(),
                qualified: assessment.qualified(),
                quantized_spacing: assessment
                    .required_conversions()
                    .contains(&RequiredConversion::QuantizedSpacing),
                restrictions: assessment.unsupported().iter().map(restriction).collect(),
            })
        })
        .collect::<Result<_>>()?;
    Ok(widget::Resource { name, presets })
}
fn restriction(feature: &adoption::UnsupportedFeature) -> &'static str {
    use adoption::UnsupportedFeature::*;
    match feature {
        ComputedTip | DynamicTip | MissingSample | AmbiguousSampleId => {
            "An unambiguous sampled tip is required."
        }
        HighDepthMask => "This tip uses unsupported mask precision.",
        SizeOverride | Rotation | Roundness | Flip => "This preset transforms the original tip.",
        DisabledSpacing | SpacingRange | OuterSpacing => "This preset uses unsupported spacing.",
        _ => "This preset includes unsupported brush behavior.",
    }
}
#[cfg(test)]
#[path = "brushes/tests.rs"]
mod tests;
