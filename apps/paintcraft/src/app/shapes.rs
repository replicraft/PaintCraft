//! Studio shape assets stay independent of document publication and save state.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    csh,
    registry::{self, Family, Operation, Platform, Request},
    save,
};
use paintcraft_ui::{shape_assets as widget, t};
#[path = "shapes/placement.rs"]
mod placement;

#[derive(Debug)]
struct Loaded {
    library: Arc<csh::Library>,
    view: widget::Resource,
    source: PathBuf,
    identity: crate::destination::SourceIdentity,
}
#[derive(Debug)]
struct Pick {
    kind: Kind,
    epoch: Uuid,
    source: Option<Arc<Loaded>>,
    bytes: Option<Vec<u8>>,
}
#[derive(Debug, Default)]
pub(super) struct State {
    loaded: Option<Arc<Loaded>>,
    epoch: Uuid,
    picker: Option<Pick>,
    error: Option<String>,
    selected: usize,
    placement: Option<placement::Draft>,
}
impl State {
    pub(super) fn pending_source_path(&self) -> Option<&Path> {
        self.placement.as_ref().map(|draft| draft.source_path())
    }
    pub(super) fn pending_source_identity(&self) -> Option<&crate::destination::SourceIdentity> {
        self.placement.as_ref().map(|draft| draft.source_identity())
    }
    pub(super) fn source_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|loaded| loaded.source.as_path())
    }
    pub(super) fn source_identity(&self) -> Option<&crate::destination::SourceIdentity> {
        self.loaded.as_ref().map(|loaded| &loaded.identity)
    }
}
fn admit(frontend: registry::Frontend, path: &Path, operation: Operation) -> Result<()> {
    let mut request = Request::new(frontend, operation, Family::Csh, Platform::Native);
    request.extension = Some(path.extension().and_then(|s| s.to_str()).unwrap_or(""));
    let decision = registry::admission(request);
    if decision.available() {
        Ok(())
    } else {
        decision.require_allowed(request)
    }
}
impl App {
    pub(super) fn begin_shapes(&mut self) {
        if self.experience() != Experience::Studio {
            return;
        }
        self.shapes.placement = None;
        self.shapes.epoch = Uuid::new_v4();
        self.shapes.picker = None;
        self.shapes.error = None;
        self.confirm = Some(Confirm::Shapes);
    }
    fn capture_shape_picker(&self, kind: Kind) -> Result<Pick> {
        if self.experience() != Experience::Studio || self.confirm != Some(Confirm::Shapes) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "shape workspace is not active",
            ));
        }
        let source = if kind == Kind::ExportShape {
            self.shapes.loaded.clone()
        } else {
            None
        };
        let bytes = source
            .as_ref()
            .map(|source| csh::encode(&source.library, &|| Ok(())))
            .transpose()?;
        if kind == Kind::ExportShape && source.is_none() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "import a shape before exporting",
            ));
        }
        Ok(Pick {
            kind,
            epoch: self.shapes.epoch,
            source,
            bytes,
        })
    }
    fn request_shape_picker(&mut self, ctx: &egui::Context, kind: Kind) {
        let result: Result<()> = (|| {
            let pick = self.capture_shape_picker(kind)?;
            let suggested = pick
                .source
                .as_ref()
                .map(|source| source.source.with_file_name("shape-copy.csh"));
            let dialogs = self
                .dialogs
                .as_mut()
                .filter(|dialogs| !dialogs.pending())
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "native shape file picker is unavailable",
                    )
                })?;
            self.shapes.picker = Some(pick);
            let repaint = ctx.clone();
            dialogs.request_with_filters(
                kind,
                suggested.as_deref(),
                &[paintcraft_platform::dialogs::Filter {
                    name: "Adobe custom shapes".into(),
                    extensions: vec!["csh".into()],
                }],
                |source| t!(ctx, source),
                move || repaint.request_repaint(),
            );
            Ok(())
        })();
        if let Err(error) = result {
            self.shapes.error = Some(t!(
                ctx,
                if kind == Kind::ImportShape {
                    "Could not import the shape: { $detail }"
                } else {
                    "Could not export the shape: { $detail }"
                },
                detail = error
            ));
        }
    }
    pub(super) fn install_shape(&mut self, path: PathBuf) -> Result<()> {
        admit(self.format_frontend(), &path, Operation::ImportAsset)?;
        self.guard_asset_adoption(&path, None)?;
        let acquired =
            crate::format_admission::acquire_resource_source(&path, csh::BYTES_MAX as u64)?;
        self.install_shape_source(acquired, path)
    }
    fn install_shape_source(
        &mut self,
        acquired: crate::format_admission::ResourceSource,
        path: PathBuf,
    ) -> Result<()> {
        admit(self.format_frontend(), &path, Operation::ImportAsset)?;
        self.guard_asset_adoption(&path, Some(&acquired.identity))?;
        let library = Arc::new(csh::decode(&acquired.bytes, &|| Ok(()))?);
        let view = widget::Resource {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            shapes: library.shapes().iter().map(shape_view).collect(),
        };
        self.shapes.epoch = Uuid::new_v4();
        self.shapes.loaded = Some(Arc::new(Loaded {
            library,
            view,
            source: path,
            identity: acquired.identity,
        }));
        self.shapes.error = None;
        self.shapes.selected = 0;
        Ok(())
    }
    pub(super) fn complete_shape_picker(
        &mut self,
        ctx: &egui::Context,
        kind: Kind,
        path: Option<PathBuf>,
    ) {
        let Some(pick) = self.shapes.picker.take() else {
            return;
        };
        if self.experience() != Experience::Studio
            || self.confirm != Some(Confirm::Shapes)
            || pick.kind != kind
            || pick.epoch != self.shapes.epoch
            || (kind == Kind::ExportShape
                && !matches!((&pick.source,&self.shapes.loaded),(Some(a),Some(b)) if Arc::ptr_eq(a,b)))
        {
            self.shapes.error = Some(t!(ctx, "The shape changed; choose the file again."));
            return;
        }
        let Some(path) = path else {
            self.editor.set_status(t!(
                ctx,
                if kind == Kind::ImportShape {
                    "Shape import cancelled."
                } else {
                    "Shape export cancelled."
                }
            ));
            return;
        };
        let result: Result<()> = (|| {
            if kind == Kind::ImportShape {
                admit(self.format_frontend(), &path, Operation::ImportAsset)?;
                self.install_shape(path)?;
            } else {
                admit(self.format_frontend(), &path, Operation::ExportAsset)?;
                self.guard_export_destination(&path)?;
                pick.source.as_ref().ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "missing shape export source")
                })?;
                let expected = match std::fs::symlink_metadata(&path) {
                    Ok(_) => {
                        let resolved = std::fs::canonicalize(&path)
                            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))?;
                        let bytes = crate::format_admission::read_source_bounded(
                            &resolved,
                            csh::BYTES_MAX as u64,
                        )?;
                        Some(save::Expectation::acquired(&resolved, &bytes))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => {
                        return Err(Error::new(ErrorCode::StorageFailed, error.to_string()));
                    }
                };
                let bytes = pick.bytes.ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "missing prepared shape output")
                })?;
                let outcome = save::save_with_expectation_and_preparation(
                    &path,
                    expected.is_some(),
                    csh::BYTES_MAX as u64,
                    expected.as_ref(),
                    |sink| {
                        std::io::Write::write_all(sink, &bytes)
                            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))
                    },
                    |_| Ok(()),
                );
                if !outcome.is_success() {
                    return Err(Error::new(
                        ErrorCode::StorageFailed,
                        outcome.reasons.join("; "),
                    ));
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.shapes.error = None;
                self.editor.set_status(t!(
                    ctx,
                    if kind == Kind::ImportShape {
                        "Shape imported."
                    } else {
                        "Shape exported."
                    }
                ));
            }
            Err(error) => {
                self.shapes.error = Some(t!(
                    ctx,
                    if kind == Kind::ImportShape {
                        "Could not import the shape: { $detail }"
                    } else {
                        "Could not export the shape: { $detail }"
                    },
                    detail = error
                ))
            }
        }
    }
    pub(super) fn shape_dialog(&mut self, ctx: &egui::Context) {
        if self.experience() != Experience::Studio {
            self.confirm = None;
            self.shapes.picker = None;
            self.shapes.placement = None;
            return;
        }
        if self.shapes.placement.is_some() {
            self.shape_placement_dialog(ctx);
            return;
        }
        match widget::dialog(
            ctx,
            self.shapes.loaded.as_ref().map(|source| &source.view),
            self.shapes.picker.is_some(),
            self.shapes.error.as_deref(),
            self.shapes.selected,
        ) {
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
                self.confirm = None;
                self.shapes.epoch = Uuid::new_v4();
            }
            widget::Response::Import => self.request_shape_picker(ctx, Kind::ImportShape),
            widget::Response::Export => self.request_shape_picker(ctx, Kind::ExportShape),
            widget::Response::Select(index) => self.shapes.selected = index,
        }
    }
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
#[path = "shapes/tests.rs"]
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
