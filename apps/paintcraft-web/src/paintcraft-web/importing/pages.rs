//! Explicit DCX selection retains encoded input and the picker owner's source.
use super::*;
use crate::editor::MAX_PIXELS;
use eframe::egui;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    dcx,
    registry::{self, Family, Frontend, Platform},
};
use paintcraft_ui::page_import::{self, Page, Response};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub(crate) struct Source {
    bytes: Arc<Vec<u8>>,
    name: String,
    pages: Vec<Page>,
    profile: Frontend,
}
impl Source {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn new(bytes: Vec<u8>, name: String, profile: Frontend) -> Result<Self> {
        if bytes.len() > 64 << 20 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Browser input limit is 64 MiB",
            ));
        }
        if profile != Frontend::Studio
            || name.rsplit('.').next().and_then(Family::from_extension) != Some(Family::Dcx)
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "DCX requires an explicit Studio page import",
            ));
        }
        let directory = dcx::inspect(&bytes)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(directory.len())
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot retain page inventory"))?;
        pages.extend(directory.into_iter().map(|page| Page {
            index: page.index,
            width: page.width,
            height: page.height,
        }));
        Ok(Self {
            bytes: Arc::new(bytes),
            name,
            pages,
            profile,
        })
    }
}
#[derive(Debug)]
pub(crate) struct Pending {
    source: Source,
    owner: Owner,
    selected: Option<u16>,
    error: Option<String>,
}
impl WebApp {
    pub(crate) fn accept_pages(&mut self, source: Source, ctx: &egui::Context) {
        if self.pending_pages.is_some() || self.pending_import.is_some() {
            return;
        }
        let Some(owner) = self.import_owner.take().filter(|owner| owner.current(self)) else {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        };
        if source.profile != self.format_profile() {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        self.pending_pages = Some(Pending {
            source,
            owner,
            selected: None,
            error: None,
        });
    }
    pub(crate) fn page_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_pages.take() else {
            return;
        };
        if !pending.owner.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let response = page_import::dialog(
            ctx,
            &pending.source.name,
            &pending.source.pages,
            &mut pending.selected,
            MAX_PIXELS,
            pending.error.as_deref(),
        );
        self.pending_pages = Some(pending);
        match response {
            Response::Pending => {}
            Response::Cancel => self.cancel_page_import(ctx),
            Response::Import(index) => self.choose_page_import(ctx, index),
        }
    }
    pub(crate) fn cancel_page_import(&mut self, ctx: &egui::Context) {
        self.pending_pages = None;
        self.message = t!(ctx, "Import cancelled.");
    }
    pub(crate) fn choose_page_import(&mut self, ctx: &egui::Context, index: u16) {
        let Some(mut pending) = self.pending_pages.take() else {
            return;
        };
        if !pending.owner.current(self) || pending.source.profile != self.format_profile() {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let result = (|| {
            registry::admit_import_page(
                pending.source.profile,
                &pending.source.name,
                &pending.source.bytes,
                Platform::Web,
                index,
            )?;
            let decoded = dcx::decode_page(&pending.source.bytes, index, MAX_PIXELS)?;
            if decoded
                .losses
                .iter()
                .any(|loss| paintcraft_ui::i18n::import_loss_description(loss).is_none())
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "unknown page import loss",
                ));
            }
            let digest = paintcraft_document::hex(&Sha256::digest(pending.source.bytes.as_slice()));
            Ok(Loaded {
                converted_copy: true,
                document: paintcraft_document::import_image(
                    decoded.width,
                    decoded.height,
                    &decoded.rgba,
                    &digest,
                )?,
                name: pending.source.name.clone(),
                losses: decoded.losses,
            })
        })();
        match result {
            Ok(loaded) => {
                self.pending_import = Some(super::Pending {
                    loaded,
                    owner: pending.owner,
                });
            }
            Err(error) => {
                pending.error = Some(error.to_string());
                self.pending_pages = Some(pending);
            }
        }
    }
}

#[cfg(test)]
mod tests;
