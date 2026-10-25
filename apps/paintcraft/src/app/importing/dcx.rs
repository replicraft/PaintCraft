//! DCX freezes source bytes and document ownership before asking for a page.
use super::*;
use paintcraft_formats::{
    dcx,
    registry::{self, Family, Platform},
};
use paintcraft_ui::page_import::{self, Page, Response};

pub(super) struct Pages {
    bytes: Arc<Vec<u8>>,
    pages: Vec<Page>,
    selected: Option<u16>,
    error: Option<String>,
}
impl App {
    pub(crate) fn stage_dcx_import(&mut self, bytes: Vec<u8>, path: PathBuf) -> Result<()> {
        if self.format_frontend() != Frontend::Studio
            || path
                .extension()
                .and_then(|s| s.to_str())
                .and_then(Family::from_extension)
                != Some(Family::Dcx)
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
        let digest = hex(&Sha256::digest(&bytes));
        self.stage_candidate(
            Candidate::Pages(Pages {
                bytes: Arc::new(bytes),
                pages,
                selected: None,
                error: None,
            }),
            path,
            digest,
            true,
        )
    }
    pub(super) fn dcx_page_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Pages(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let name = pending
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let response = page_import::dialog(
            ctx,
            &name,
            &source.pages,
            &mut source.selected,
            image::DECODE_PIXELS_MAX,
            source.error.as_deref(),
        );
        self.pending_import = Some(pending);
        match response {
            Response::Pending => {}
            Response::Cancel => self.resolve_import(ctx, false),
            Response::Import(index) => self.choose_dcx_page(ctx, index),
        }
    }
    pub(super) fn choose_dcx_page(&mut self, ctx: &egui::Context, index: u16) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Pages(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let result = (|| {
            registry::admit_import_page(
                pending.frontend,
                &pending.path.to_string_lossy(),
                &source.bytes,
                Platform::Native,
                index,
            )?;
            let image = dcx::decode_page(&source.bytes, index, image::DECODE_PIXELS_MAX)?;
            if image
                .losses
                .iter()
                .any(|loss| import_loss_description(loss).is_none())
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "unknown page import loss",
                ));
            }
            Ok(image)
        })();
        match result {
            Ok(image) => {
                pending.candidate = Candidate::Raster(image);
                self.pending_import = Some(pending);
                self.confirm = Some(Confirm::ImportLosses);
            }
            Err(error) => {
                source.error = Some(error.to_string());
                self.pending_import = Some(pending);
            }
        }
    }
}

#[cfg(test)]
#[path = "dcx/tests.rs"]
mod tests;
