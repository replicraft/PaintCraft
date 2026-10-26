//! Selected ICNS input retains the exact acquired file through preview and consent.
use super::*;
use paintcraft_formats::{
    icns,
    registry::{self, Family, Platform},
};
use paintcraft_ui::{icns_import as widget, icon_export::Frame};
pub(super) struct Source {
    bytes: Vec<u8>,
    entries: Vec<widget::Entry>,
    state: widget::State<icns::Decoded>,
}
fn entry(value: icns::Entry) -> widget::Entry {
    widget::Entry {
        index: value.index,
        record: value.record,
        tag: String::from_utf8_lossy(&value.tag).into_owned(),
        logical_width: value.logical_width,
        logical_height: value.logical_height,
        scale: value.scale,
        width: value.width,
        height: value.height,
        encoding: match value.encoding {
            icns::Encoding::Png => "PNG",
            icns::Encoding::LegacyRaw => "RGB and transparency mask",
            icns::Encoding::LegacyRle => "Compressed RGB and transparency mask",
            icns::Encoding::Unqualified => "Unsupported encoding",
        },
        qualified: value.encoding != icns::Encoding::Unqualified,
    }
}
impl App {
    pub(crate) fn stage_icns_import_acquired(
        &mut self,
        source: crate::format_admission::ResourceSource,
        path: PathBuf,
    ) -> Result<()> {
        if self.format_frontend() != Frontend::Studio
            || registry::sniff(&source.bytes) != Some(Family::Icns)
            || path
                .extension()
                .and_then(|s| s.to_str())
                .and_then(Family::from_extension)
                != Some(Family::Icns)
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "ICNS requires an explicit Studio entry import",
            ));
        }
        if path
            .file_name()
            .is_none_or(|name| name.to_string_lossy().len() > 4096)
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "ICNS filename exceeds allowance",
            ));
        }
        self.guard_asset_adoption(&path, Some(&source.identity))?;
        let directory = icns::inspect(&source.bytes, &|| Ok(()))?;
        let digest = hex(&Sha256::digest(&source.bytes));
        self.stage_candidate(
            Candidate::Icns(Box::new(Source {
                bytes: source.bytes,
                entries: directory.entries.into_iter().map(entry).collect(),
                state: Default::default(),
            })),
            path.clone(),
            digest,
            true,
        )?;
        self.pending_import
            .as_mut()
            .unwrap()
            .auxiliary_sources
            .push(crate::destination::SourceGuard {
                path,
                identity: source.identity,
            });
        Ok(())
    }
    pub(super) fn icns_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Icns(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let response = source.state.dialog(
            ctx,
            &pending
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            &source.entries,
            image::DECODE_PIXELS_MAX,
            |decoded| Frame {
                width: decoded.entry.width,
                height: decoded.entry.height,
                rgba: &decoded.rgba,
                hotspot: None,
            },
            |decoded| &decoded.losses,
        );
        match response {
            widget::Response::Pending => {}
            widget::Response::Cancel => {
                self.confirm = None;
                self.editor.set_status(t!(ctx, "Import cancelled."));
                return;
            }
            widget::Response::Preview(index) => {
                source.state.prepare(ctx, index, || {
                    registry::admit_import_entry(
                        pending.frontend,
                        &pending.path.to_string_lossy(),
                        &source.bytes,
                        Platform::Native,
                        index,
                    )
                    .map_err(|e| e.to_string())?;
                    icns::decode_entry(&source.bytes, index, image::DECODE_PIXELS_MAX, &|| Ok(()))
                        .map_err(|e| e.to_string())
                });
            }
            widget::Response::Import(index) => {
                let converted = (|| {
                    let decoded = source
                        .state
                        .value()
                        .filter(|decoded| decoded.entry.index == index)
                        .ok_or_else(|| {
                            Error::new(ErrorCode::ConflictStaleBase, "ICNS entry preview changed")
                        })?;
                    for guard in &pending.auxiliary_sources {
                        self.guard_asset_adoption(&guard.path, Some(&guard.identity))?;
                    }
                    Ok::<_, Error>(paintcraft_formats::project_import::Converted {
                        document: decoded.document(&pending.digest)?,
                        losses: decoded.losses.clone(),
                    })
                })();
                match converted {
                    Ok(converted) => {
                        pending.candidate = Candidate::Layered(converted);
                        self.pending_import = Some(pending);
                        self.resolve_import(ctx, true);
                        return;
                    }
                    Err(error) => {
                        self.editor.set_status(format!("Cannot open: {error}"));
                    }
                }
            }
        }
        self.pending_import = Some(pending);
    }
}
#[cfg(test)]
#[path = "icns/tests.rs"]
mod tests;
