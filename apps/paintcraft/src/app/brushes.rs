//! Explicit static-tip adoption retains the acquired source with its document.
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
#[derive(Debug)]
struct Loaded {
    id: Uuid,
    library: Arc<abr::Library>,
    view: widget::Resource,
    source: crate::destination::SourceGuard,
}
#[derive(Debug, Default)]
pub(super) struct State {
    owner: Option<(Uuid, (Uuid, u64))>,
    loaded: Option<Arc<Loaded>>,
    epoch: Uuid,
    pick: Option<Uuid>,
    selected: usize,
    controls: widget::State,
    error: Option<String>,
}
impl State {
    pub(super) fn source_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|s| s.source.path.as_path())
    }
    pub(super) fn source_identity(&self) -> Option<&crate::destination::SourceIdentity> {
        self.loaded.as_ref().map(|s| &s.source.identity)
    }
}
fn admit(path: &Path) -> Result<()> {
    let mut request = Request::new(
        registry::Frontend::Studio,
        Operation::ImportAsset,
        Family::Abr,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|s| s.to_str());
    registry::admission(request).require_allowed(request)
}
impl App {
    pub(super) fn begin_brushes(&mut self) {
        if self.experience() != Experience::Studio {
            return;
        }
        self.brushes.owner = Some((self.active_tab_id(), self.editor.render_key()));
        self.brushes.epoch = Uuid::new_v4();
        self.brushes.pick = None;
        self.brushes.controls.reset();
        self.brushes.error = None;
        self.confirm = Some(Confirm::Brushes);
    }
    fn request_brush_picker(&mut self, ctx: &egui::Context) -> Result<()> {
        let dialogs = self
            .dialogs
            .as_mut()
            .filter(|d| !d.pending())
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Native brush picker unavailable",
                )
            })?;
        self.brushes.pick = Some(self.brushes.epoch);
        let repaint = ctx.clone();
        dialogs.request_with_filters(
            Kind::ImportBrush,
            None,
            &[paintcraft_platform::dialogs::Filter {
                name: "Adobe brushes".into(),
                extensions: vec!["abr".into()],
            }],
            |s| t!(ctx, s),
            move || repaint.request_repaint(),
        );
        Ok(())
    }
    fn install_brushes(&mut self, path: PathBuf) -> Result<()> {
        if path.file_name().unwrap_or_default().to_string_lossy().len() > 4096 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Brush filename exceeds allowance",
            ));
        }
        admit(&path)?;
        self.guard_asset_adoption(&path, None)?;
        let source =
            crate::format_admission::acquire_resource_source(&path, abr::BYTES_MAX as u64)?;
        self.guard_asset_adoption(&path, Some(&source.identity))?;
        let library = Arc::new(abr::decode(&source.bytes, &|| Ok(()))?);
        let view = brush_view(
            &library,
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )?;
        self.brushes.loaded = Some(Arc::new(Loaded {
            id: paintcraft_core::types::ids::deterministic_id(&[
                "brush-library",
                "abr",
                &Sha256::digest(&source.bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            ]),
            library,
            view,
            source: crate::destination::SourceGuard {
                path,
                identity: source.identity,
            },
        }));
        self.brushes.epoch = Uuid::new_v4();
        self.brushes.selected = 0;
        self.brushes.controls.reset();
        self.brushes.error = None;
        Ok(())
    }
    pub(super) fn complete_brush_picker(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        let Some(epoch) = self.brushes.pick.take() else {
            return;
        };
        if self.experience() != Experience::Studio
            || self.confirm != Some(Confirm::Brushes)
            || epoch != self.brushes.epoch
            || self.brushes.owner != Some((self.active_tab_id(), self.editor.render_key()))
        {
            return;
        }
        if let Some(path) = path
            && let Err(error) = self.install_brushes(path)
        {
            self.brushes.error = Some(t!(
                ctx,
                "Could not import brushes: { $detail }",
                detail = error
            ));
        }
    }
    fn adopt_brush(&mut self) -> Result<()> {
        let source = self
            .brushes
            .loaded
            .as_ref()
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "No brush resource loaded"))?;
        self.guard_asset_adoption(&source.source.path, Some(&source.source.identity))?;
        let acknowledged = if self.brushes.controls.consent {
            &[RequiredConversion::QuantizedSpacing][..]
        } else {
            &[]
        };
        let adopted = adoption::adopt_native(
            source.library.clone(),
            self.brushes.selected,
            StaticTipPolicy::NativeGridV1,
            acknowledged,
            &SharedBudget::new(64 << 20),
            &|| Ok(()),
        )?;
        let name = source.view.presets[self.brushes.selected].name.clone();
        self.editor
            .use_sampled_tip(Some((adopted.tip().clone(), adopted.spacing_q8())));
        self.editor.adopted_brush = Some(adopted);
        self.editor.set_sampled_source(Some(source.id));
        self.editor.set_sampled_brush_name(Some(name));
        // The document owns acquired inputs even after the library is replaced.
        if !self
            .auxiliary_sources
            .iter()
            .any(|old| old.identity.matches(&source.source.identity))
        {
            self.auxiliary_sources.push(source.source.clone());
        }
        self.confirm = None;
        Ok(())
    }
    pub(super) fn brush_dialog(&mut self, ctx: &egui::Context) {
        if self.brushes.owner != Some((self.active_tab_id(), self.editor.render_key())) {
            self.confirm = None;
            self.brushes.pick = None;
            self.brushes.epoch = Uuid::new_v4();
            return;
        }
        let result = match self.brushes.controls.dialog(
            ctx,
            self.brushes.loaded.as_ref().map(|s| &s.view),
            self.brushes.selected,
            self.brushes.pick.is_some(),
            self.brushes.error.as_deref(),
            self.editor.sampled_brush_name(),
        ) {
            widget::Response::Pending => Ok(()),
            widget::Response::Close => {
                self.confirm = None;
                Ok(())
            }
            widget::Response::Import => self.request_brush_picker(ctx),
            widget::Response::Select(index) => {
                self.brushes.selected = index;
                self.brushes.controls.reset();
                Ok(())
            }
            widget::Response::Stock => {
                self.editor.adopted_brush = None;
                self.editor.use_sampled_tip(None);
                self.editor.set_sampled_source(None);
                self.editor.set_sampled_brush_name(None);
                self.confirm = None;
                Ok(())
            }
            widget::Response::Adopt => self.adopt_brush(),
        };
        if let Err(error) = result {
            self.brushes.error = Some(t!(
                ctx,
                "Could not use this brush: { $detail }",
                detail = error
            ));
        }
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
