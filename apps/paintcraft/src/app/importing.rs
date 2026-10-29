//! Loss consent retains decoded source pixels, never a published replacement.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::registry::Frontend;
use paintcraft_ui::{i18n::import_loss_description, t};

#[path = "importing/dcx.rs"]
mod dcx;
#[path = "importing/dds.rs"]
mod dds;
#[path = "importing/icns.rs"]
mod icns;
#[path = "importing/icons.rs"]
mod icons;
#[path = "importing/msp.rs"]
mod msp;
#[path = "importing/native_codec.rs"]
mod native_codec;
#[path = "importing/ora.rs"]
mod ora;
#[path = "importing/stock.rs"]
mod stock;
#[path = "importing/svg.rs"]
mod svg;
#[path = "importing/tiff.rs"]
pub(crate) mod tiff;
pub(super) use svg::FontPick;

enum Candidate {
    Pages(dcx::Pages),
    Tiff(Box<tiff::Source>),
    Entries(icons::Entries),
    Icns(Box<icns::Source>),
    NativeCodec(Box<native_codec::Source>),
    Svg(Box<svg::Source>),
    Raster(image::Decoded),
    Layered(paintcraft_formats::project_import::Converted),
}
impl Candidate {
    fn losses(&self) -> &[String] {
        match self {
            Self::Pages(_)
            | Self::Tiff(_)
            | Self::Entries(_)
            | Self::Icns(_)
            | Self::NativeCodec(_)
            | Self::Svg(_) => &[],
            Self::Raster(image) => &image.losses,
            Self::Layered(project) => &project.losses,
        }
    }
}
struct CapturedSource {
    identity: crate::destination::SourceIdentity,
    bytes: u64,
    sha256: [u8; 32],
}
pub(super) struct Pending {
    candidate: Candidate,
    path: PathBuf,
    digest: String,
    raster_key: Option<paintcraft_document::RasterConversionKey>,
    owner: Uuid,
    base: Arc<paintcraft_document::Document>,
    ticket: (Uuid, u64),
    frontend: Frontend,
    dirty: bool,
    convert: bool,
    auxiliary_sources: Vec<crate::destination::SourceGuard>,
    captured_source: Option<CapturedSource>,
}
impl Pending {
    pub(super) fn input_guards(&self) -> Vec<&crate::destination::SourceGuard> {
        let mut inputs: Vec<_> = self.auxiliary_sources.iter().collect();
        if let Candidate::Svg(source) = &self.candidate {
            inputs.extend(source.input_guards());
        }
        inputs
    }
    fn current(&self, app: &App) -> bool {
        app.active_tab_id() == self.owner
            && app.editor.render_key() == self.ticket
            && Arc::ptr_eq(&app.editor.document_snapshot(), &self.base)
            && app.format_frontend() == self.frontend
            && !app.has_document_drafts()
            && app.editor.is_dirty() == self.dirty
            && !app.editor.has_pending_edits()
            && !app.editor.has_floating_selection()
    }
}
impl App {
    pub(super) fn stage_import(
        &mut self,
        image: image::Decoded,
        path: PathBuf,
        digest: String,
        convert: bool,
    ) -> Result<()> {
        self.stage_candidate(Candidate::Raster(image), path, digest, convert)
    }
    pub(super) fn stage_project_import(
        &mut self,
        project: paintcraft_formats::project_import::Converted,
        path: PathBuf,
    ) -> Result<()> {
        self.stage_candidate(Candidate::Layered(project), path, String::new(), true)
    }
    fn stage_candidate(
        &mut self,
        candidate: Candidate,
        path: PathBuf,
        digest: String,
        convert: bool,
    ) -> Result<()> {
        if self.pending_import.is_some() {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "An image import is already awaiting confirmation",
            ));
        }
        if candidate
            .losses()
            .iter()
            .any(|loss| import_loss_description(loss).is_none())
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Image import contains an unrecognized loss",
            ));
        }
        self.pending_import = Some(Pending {
            candidate,
            path,
            digest,
            raster_key: None,
            owner: self.active_tab_id(),
            base: self.editor.document_snapshot(),
            ticket: self.editor.render_key(),
            frontend: self.format_frontend(),
            dirty: self.editor.is_dirty(),
            convert,
            auxiliary_sources: Vec::new(),
            captured_source: None,
        });
        self.confirm = Some(Confirm::ImportLosses);
        Ok(())
    }
    pub(super) fn resolve_import(&mut self, ctx: &egui::Context, accept: bool) {
        let Some(pending) = self.pending_import.take() else {
            self.confirm = None;
            return;
        };
        self.confirm = None;
        if !accept {
            self.editor.set_status(t!(ctx, "Import cancelled."));
            return;
        }
        if !pending.current(self) {
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        if matches!(
            pending.candidate,
            Candidate::Pages(_)
                | Candidate::Tiff(_)
                | Candidate::Entries(_)
                | Candidate::Icns(_)
                | Candidate::NativeCodec(_)
                | Candidate::Svg(_)
        ) {
            self.pending_import = Some(pending);
            self.confirm = Some(Confirm::ImportLosses);
            return;
        }
        let result = (|| {
            if let Some(source) = &pending.captured_source {
                source.identity.verify_bytes(
                    &pending.path,
                    source.bytes,
                    &source.sha256,
                    &|| Ok(()),
                )?;
            }
            let converted = pending.convert
                || matches!(&pending.candidate, Candidate::Layered(_))
                || matches!(&pending.candidate, Candidate::Raster(image) if image.format == ImageFormat::Dds && !image.losses.is_empty());
            let document = match pending.candidate {
                Candidate::Raster(image) => import_image(
                    image.width,
                    image.height,
                    &image.rgba,
                    &pending
                        .raster_key
                        .map(|key| key.hex())
                        .unwrap_or(pending.digest),
                )?,
                Candidate::Layered(project) => project.document,
                Candidate::Pages(_)
                | Candidate::Tiff(_)
                | Candidate::Entries(_)
                | Candidate::Icns(_)
                | Candidate::NativeCodec(_)
                | Candidate::Svg(_) => {
                    unreachable!("entry selection is resolved separately")
                }
            };
            let mut editor = Editor::with_document(document, None);
            if converted {
                editor.invalidate_saved_state();
            }
            if let Some(source) = &pending.captured_source {
                source.identity.verify_bytes(
                    &pending.path,
                    source.bytes,
                    &source.sha256,
                    &|| Ok(()),
                )?;
            }
            self.install(editor)?;
            self.bound = None;
            self.imported_source = Some(pending.path.clone());
            self.auxiliary_sources = pending.auxiliary_sources;
            self.path_text = pending
                .path
                .with_extension("pcnd")
                .to_string_lossy()
                .into_owned();
            self.editor
                .set_status(format!("Imported {}", pending.path.display()));
            if !pending.convert {
                self.recent.remember(&pending.path);
            }
            Ok::<(), Error>(())
        })();
        if let Err(error) = result {
            self.editor.set_status(format!("Cannot open: {error}"));
        }
    }
    pub(super) fn import_dialog(&mut self, ctx: &egui::Context) {
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| matches!(pending.candidate, Candidate::Tiff(_)))
        {
            self.tiff_import_dialog(ctx);
            return;
        }
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| matches!(pending.candidate, Candidate::NativeCodec(_)))
        {
            self.native_codec_dialog(ctx);
            return;
        }
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| matches!(pending.candidate, Candidate::Icns(_)))
        {
            self.icns_import_dialog(ctx);
            return;
        }
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| matches!(pending.candidate, Candidate::Svg(_)))
        {
            self.svg_import_dialog(ctx);
            return;
        }
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| matches!(pending.candidate, Candidate::Entries(_)))
        {
            self.icon_entry_dialog(ctx);
            return;
        }
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| matches!(pending.candidate, Candidate::Pages(_)))
        {
            self.dcx_page_dialog(ctx);
            return;
        }
        let Some(pending) = &self.pending_import else {
            self.confirm = None;
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("native-import-losses")).show(ctx, |ui| {
            ui.heading(t!(ctx, "Open with these import limitations?"));
            for loss in pending.candidate.losses() {
                if let Some(description) = import_loss_description(loss) {
                    ui.label(t!(ctx, description));
                }
            }
            ui.horizontal(|ui| {
                accept = ui.button(t!(ctx, "Open image")).clicked();
                cancel = ui.button(t!(ctx, "Cancel")).clicked();
            });
        });
        if ctx.current_pass_index() == 0
            && (accept || cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            self.resolve_import(ctx, accept);
        }
    }
}

#[cfg(test)]
#[path = "importing/psd_tests.rs"]
mod psd_tests;
#[cfg(test)]
#[path = "importing/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "importing/stock_tests.rs"]
mod stock_tests;
