//! Frozen ICO/CUR bytes remain private until entry selection and loss consent.
use super::*;
use paintcraft_formats::{
    icons,
    registry::{self, Family, Platform},
};
use paintcraft_ui::{entry_import, page_import::Response};

pub(super) struct Entries {
    bytes: Vec<u8>,
    entries: Vec<entry_import::Entry>,
    selected: Option<u16>,
    error: Option<String>,
}

#[cfg(test)]
#[path = "icons/tests.rs"]
mod tests;
impl App {
    pub(crate) fn stage_icon_import(&mut self, bytes: Vec<u8>, path: PathBuf) -> Result<()> {
        let family = registry::sniff(&bytes);
        if self.format_frontend() != Frontend::Studio
            || !matches!(family, Some(Family::Ico | Family::Cur))
            || path
                .extension()
                .and_then(|s| s.to_str())
                .and_then(Family::from_extension)
                != family
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "icons require an explicit Studio entry import",
            ));
        }
        let entries = icons::inspect(&bytes)?
            .entries
            .into_iter()
            .map(|entry| entry_import::Entry {
                index: entry.index,
                width: entry.width,
                height: entry.height,
                encoding: if entry.encoding == icons::Encoding::Png {
                    "PNG"
                } else {
                    "DIB"
                },
                depth: entry.bits_per_pixel,
                hotspot: entry.hotspot,
            })
            .collect();
        let digest = hex(&Sha256::digest(&bytes));
        self.stage_candidate(
            Candidate::Entries(Entries {
                bytes,
                entries,
                selected: None,
                error: None,
            }),
            path,
            digest,
            true,
        )
    }
    pub(super) fn icon_entry_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Entries(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let response = entry_import::dialog(
            ctx,
            &pending
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            &source.entries,
            &mut source.selected,
            image::DECODE_PIXELS_MAX,
            source.error.as_deref(),
        );
        self.pending_import = Some(pending);
        match response {
            Response::Pending => {}
            Response::Cancel => self.resolve_import(ctx, false),
            Response::Import(index) => self.choose_icon_entry(ctx, index),
        }
    }
    pub(super) fn choose_icon_entry(&mut self, ctx: &egui::Context, index: u16) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Entries(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let result = (|| {
            registry::admit_import_entry(
                pending.frontend,
                &pending.path.to_string_lossy(),
                &source.bytes,
                Platform::Native,
                index,
            )?;
            let decoded = icons::decode_entry(&source.bytes, index, image::DECODE_PIXELS_MAX)?;
            if decoded
                .losses
                .iter()
                .any(|loss| import_loss_description(loss).is_none())
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "unknown icon import loss",
                ));
            }
            Ok(paintcraft_formats::project_import::Converted {
                document: decoded.document(&pending.digest)?,
                losses: decoded.losses,
            })
        })();
        match result {
            Ok(converted) => {
                pending.candidate = Candidate::Layered(converted);
                self.pending_import = Some(pending);
            }
            Err(error) => {
                source.error = Some(error.to_string());
                self.pending_import = Some(pending);
            }
        }
    }
}
