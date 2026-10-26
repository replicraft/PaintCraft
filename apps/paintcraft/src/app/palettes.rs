//! Studio palette assets stay independent of document publication and save state.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    ase,
    registry::{self, Family, Operation, Platform, Request},
    save,
};
use paintcraft_ui::{palette as widget, t};

#[derive(Debug)]
struct Loaded {
    palette: ase::Palette,
    view: widget::Resource,
    source: PathBuf,
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
}
impl State {
    pub(super) fn source_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|loaded| loaded.source.as_path())
    }
}
fn swatch(color: &ase::Swatch) -> widget::Row {
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
}
fn admit(frontend: registry::Frontend, path: &Path, operation: Operation) -> Result<()> {
    let mut request = Request::new(frontend, operation, Family::Ase, Platform::Native);
    request.extension = Some(path.extension().and_then(|s| s.to_str()).unwrap_or(""));
    let decision = registry::admission(request);
    if decision.available() {
        Ok(())
    } else {
        decision.require_allowed(request)
    }
}
impl App {
    pub(super) fn begin_palettes(&mut self) {
        if self.experience() != Experience::Studio {
            return;
        }
        self.palettes.epoch = Uuid::new_v4();
        self.palettes.picker = None;
        self.palettes.error = None;
        self.confirm = Some(Confirm::Palettes);
    }
    fn capture_palette_picker(&self, kind: Kind) -> Result<Pick> {
        if self.experience() != Experience::Studio || self.confirm != Some(Confirm::Palettes) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "palette workspace is not active",
            ));
        }
        let source = if kind == Kind::ExportPalette {
            self.palettes.loaded.clone()
        } else {
            None
        };
        let bytes = source
            .as_ref()
            .map(|source| ase::encode(&source.palette, &|| Ok(())))
            .transpose()?;
        if kind == Kind::ExportPalette && source.is_none() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "import a palette before exporting",
            ));
        }
        Ok(Pick {
            kind,
            epoch: self.palettes.epoch,
            source,
            bytes,
        })
    }
    fn request_palette_picker(&mut self, ctx: &egui::Context, kind: Kind) {
        let result: Result<()> = (|| {
            let pick = self.capture_palette_picker(kind)?;
            let suggested = pick
                .source
                .as_ref()
                .map(|source| source.source.with_file_name("palette-copy.ase"));
            let dialogs = self
                .dialogs
                .as_mut()
                .filter(|dialogs| !dialogs.pending())
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "native palette file picker is unavailable",
                    )
                })?;
            self.palettes.picker = Some(pick);
            let repaint = ctx.clone();
            dialogs.request_with_filters(
                kind,
                suggested.as_deref(),
                &[paintcraft_platform::dialogs::Filter {
                    name: "Adobe Swatch Exchange palette".into(),
                    extensions: vec!["ase".into()],
                }],
                |source| t!(ctx, source),
                move || repaint.request_repaint(),
            );
            Ok(())
        })();
        if let Err(error) = result {
            self.palettes.error = Some(t!(
                ctx,
                if kind == Kind::ImportPalette {
                    "Could not import the palette: { $detail }"
                } else {
                    "Could not export the palette: { $detail }"
                },
                detail = error
            ));
        }
    }
    pub(super) fn install_palette(&mut self, bytes: &[u8], path: PathBuf) -> Result<()> {
        admit(self.format_frontend(), &path, Operation::ImportAsset)?;
        self.guard_asset_adoption(&path)?;
        let palette = ase::decode(bytes, &|| Ok(()))?;
        let mut rows = Vec::new();
        for entry in palette.entries() {
            match entry {
                ase::Entry::Swatch(color) => rows.push(swatch(color)),
                ase::Entry::Group { name, swatches } => {
                    rows.push(widget::Row::Group(name.clone()));
                    rows.extend(swatches.iter().map(swatch));
                }
            }
        }
        let view = widget::Resource {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            rows,
        };
        self.palettes.epoch = Uuid::new_v4();
        self.palettes.loaded = Some(Arc::new(Loaded {
            palette,
            view,
            source: path,
        }));
        self.palettes.error = None;
        Ok(())
    }
    pub(super) fn complete_palette_picker(
        &mut self,
        ctx: &egui::Context,
        kind: Kind,
        path: Option<PathBuf>,
    ) {
        let Some(pick) = self.palettes.picker.take() else {
            return;
        };
        if self.experience() != Experience::Studio
            || self.confirm != Some(Confirm::Palettes)
            || pick.kind != kind
            || pick.epoch != self.palettes.epoch
            || (kind == Kind::ExportPalette
                && !matches!((&pick.source,&self.palettes.loaded),(Some(a),Some(b)) if Arc::ptr_eq(a,b)))
        {
            self.palettes.error = Some(t!(ctx, "The palette changed; choose the file again."));
            return;
        }
        let Some(path) = path else {
            self.editor.set_status(t!(
                ctx,
                if kind == Kind::ImportPalette {
                    "Palette import cancelled."
                } else {
                    "Palette export cancelled."
                }
            ));
            return;
        };
        let result: Result<()> = (|| {
            if kind == Kind::ImportPalette {
                admit(self.format_frontend(), &path, Operation::ImportAsset)?;
                let bytes =
                    crate::format_admission::read_source_bounded(&path, ase::BYTES_MAX as u64)?;
                self.install_palette(&bytes, path)?;
            } else {
                admit(self.format_frontend(), &path, Operation::ExportAsset)?;
                self.guard_export_destination(&path)?;
                if self
                    .protected_source_paths()
                    .into_iter()
                    .any(|source| crate::destination::same_file(source, &path))
                {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "palette export must not replace an open document source",
                    ));
                }
                let source = pick.source.as_ref().ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "missing palette export source")
                })?;
                if crate::destination::same_file(&source.source, &path) {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "palette export must not replace its source",
                    ));
                }
                let expected = match std::fs::symlink_metadata(&path) {
                    Ok(_) => {
                        let resolved = std::fs::canonicalize(&path)
                            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))?;
                        let bytes = crate::format_admission::read_source_bounded(
                            &resolved,
                            ase::BYTES_MAX as u64,
                        )?;
                        Some(save::Expectation::acquired(&resolved, &bytes))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => {
                        return Err(Error::new(ErrorCode::StorageFailed, error.to_string()));
                    }
                };
                let bytes = pick.bytes.ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "missing prepared palette output")
                })?;
                let outcome = save::save_with_expectation_and_preparation(
                    &path,
                    expected.is_some(),
                    ase::BYTES_MAX as u64,
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
                self.palettes.error = None;
                self.editor.set_status(t!(
                    ctx,
                    if kind == Kind::ImportPalette {
                        "Palette imported."
                    } else {
                        "Palette exported."
                    }
                ));
            }
            Err(error) => {
                self.palettes.error = Some(t!(
                    ctx,
                    if kind == Kind::ImportPalette {
                        "Could not import the palette: { $detail }"
                    } else {
                        "Could not export the palette: { $detail }"
                    },
                    detail = error
                ))
            }
        }
    }
    pub(super) fn palette_dialog(&mut self, ctx: &egui::Context) {
        if self.experience() != Experience::Studio {
            self.confirm = None;
            self.palettes.picker = None;
            return;
        }
        match widget::dialog(
            ctx,
            self.palettes.loaded.as_ref().map(|source| &source.view),
            self.palettes.picker.is_some(),
            self.palettes.error.as_deref(),
        ) {
            widget::Response::Pending => {}
            widget::Response::Close => {
                self.confirm = None;
                self.palettes.epoch = Uuid::new_v4();
            }
            widget::Response::Import => self.request_palette_picker(ctx, Kind::ImportPalette),
            widget::Response::Export => self.request_palette_picker(ctx, Kind::ExportPalette),
            widget::Response::Pick(rgba) => {
                if self.studio.edit_secondary {
                    self.editor.secondary = Rgba(rgba);
                } else {
                    self.editor.color = Rgba(rgba);
                }
                self.confirm = None;
            }
        }
    }
}
#[cfg(test)]
#[path = "palettes/tests.rs"]
mod tests;
