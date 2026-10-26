//! Frozen SVG input and its exact preview output belong to one picker owner.
use super::*;
use eframe::egui;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    registry::{self, Frontend, Platform},
    svg,
};
use paintcraft_ui::svg_import::{self, Response};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc};
mod fonts;

#[derive(Debug)]
pub(crate) struct Source {
    bytes: Vec<u8>,
    name: String,
    digest: String,
}
impl Source {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn new(bytes: Vec<u8>, name: String, profile: Frontend) -> Result<Self> {
        registry::admit_import_svg(profile, &name, &bytes, Platform::Web)?;
        let digest = paintcraft_document::hex(&Sha256::digest(&bytes));
        Ok(Self {
            bytes,
            name,
            digest,
        })
    }
}
#[derive(Debug)]
pub(crate) struct Pending {
    id: uuid::Uuid,
    font_files: uuid::Uuid,
    additional: Vec<fonts::FontFile>,
    font_busy: bool,
    font_inbox: Rc<RefCell<fonts::Inbox>>,
    source: Source,
    owner: Owner,
    fonts: Option<svg::Fonts>,
    state: svg_import::State<Loaded>,
}
impl WebApp {
    pub(crate) fn accept_svg(&mut self, source: Source, ctx: &egui::Context) {
        let owner = self.import_owner.take();
        if self.pending_import.is_some()
            || self.pending_pages.is_some()
            || self.pending_svg.is_some()
        {
            return;
        }
        let Some(owner) = owner.filter(|owner| owner.current(self)) else {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        };
        if self.format_profile() != Frontend::Studio {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        self.pending_svg = Some(Pending {
            id: uuid::Uuid::new_v4(),
            font_files: uuid::Uuid::new_v4(),
            additional: Vec::new(),
            font_busy: false,
            font_inbox: Default::default(),
            source,
            owner,
            fonts: None,
            state: Default::default(),
        });
    }
    pub(crate) fn svg_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_svg.take() else {
            return;
        };
        if !pending.owner.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        pending.accept_fonts(ctx);
        pending.sync_fonts();
        match pending.state.dialog(ctx, &pending.source.name) {
            Response::AddFonts => pending.pick_fonts(ctx),
            Response::RemoveFont(index) => pending.remove_font(index),
            Response::Preview(config) => {
                let result = (|| {
                    let options = svg::Options {
                        width: config.width,
                        height: config.height,
                        dpi: config.dpi,
                        fit: match config.fit {
                            svg_import::Fit::Contain => svg::Fit::Contain,
                            svg_import::Fit::Stretch => svg::Fit::Stretch,
                        },
                    };
                    options.admit(crate::editor::MAX_PIXELS)?;
                    if pending.fonts.is_none() {
                        pending.fonts = Some(pending.build_fonts(&[])?);
                    }
                    let raster = svg::prepare(
                        &pending.source.bytes,
                        options,
                        pending.fonts.as_ref().unwrap(),
                        crate::editor::MAX_PIXELS,
                        &|| Ok(()),
                    )?
                    .render(&|| Ok(()))?;
                    let preview = raster.preview(&|| Ok(()))?;
                    let document = raster.document(&pending.source.digest, &|| Ok(()))?;
                    let losses = raster.losses;
                    let loaded = Loaded {
                        converted_copy: true,
                        document,
                        name: pending.source.name.clone(),
                        losses: losses.clone(),
                    };
                    if !pending.state.install(
                        ctx,
                        config,
                        loaded,
                        [preview.width, preview.height],
                        &preview.rgba,
                        losses,
                    ) {
                        return Err(Error::new(
                            ErrorCode::CapabilityUnsupported,
                            "SVG preview contains an unrecognized conversion loss",
                        ));
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    pending.state.fail(error.to_string());
                }
            }
            Response::Import => {
                if let Some(loaded) = pending.state.take(ctx) {
                    self.pending_import = Some(super::Pending {
                        loaded,
                        owner: pending.owner,
                    });
                    self.resolve_import(ctx, true);
                    return;
                }
            }
            Response::Cancel => {
                self.message = t!(ctx, "Import cancelled.");
                return;
            }
            Response::Pending => {}
        }
        self.pending_svg = Some(pending);
    }
}

#[cfg(test)]
mod tests;
