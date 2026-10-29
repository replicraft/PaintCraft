//! Browser entry selection owns frozen bytes, document identity and reviewed pixels.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    icns,
    registry::{self, Family, Frontend, Platform},
};
use paintcraft_ui::{icns_import as widget, icon_export::Frame};
use sha2::{Digest, Sha256};
#[derive(Debug)]
pub(crate) struct Source {
    bytes: Vec<u8>,
    name: String,
    digest: String,
    entries: Vec<widget::Entry>,
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
impl Source {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn new(bytes: Vec<u8>, name: String, profile: Frontend) -> Result<Self> {
        if bytes.len() > icns::BYTES_MAX || name.len() > 4096 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "ICNS input exceeds allowance",
            ));
        }
        if profile != Frontend::Studio
            || registry::sniff(&bytes) != Some(Family::Icns)
            || name.rsplit('.').next().and_then(Family::from_extension) != Some(Family::Icns)
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "ICNS requires an explicit Studio entry import",
            ));
        }
        let directory = icns::inspect(&bytes, &|| Ok(()))?;
        let digest = paintcraft_document::hex(&Sha256::digest(&bytes));
        Ok(Self {
            bytes,
            name,
            digest,
            entries: directory.entries.into_iter().map(entry).collect(),
        })
    }
}
#[derive(Debug)]
pub(crate) struct Pending {
    source: Source,
    owner: Owner,
    state: widget::State<icns::Decoded>,
}
impl WebApp {
    pub(crate) fn accept_icns(&mut self, source: Source, ctx: &eframe::egui::Context) {
        let owner = self.import_owner.take();
        if self.pending_import.is_some()
            || self.pending_pages.is_some()
            || self.pending_svg.is_some()
            || self.pending_icns.is_some()
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
        self.pending_icns = Some(Pending {
            source,
            owner,
            state: Default::default(),
        });
    }
    pub(crate) fn icns_import_dialog(&mut self, ctx: &eframe::egui::Context) {
        let Some(mut pending) = self.pending_icns.take() else {
            return;
        };
        if !pending.owner.current(self) || self.format_profile() != Frontend::Studio {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let response = pending.state.dialog(
            ctx,
            &pending.source.name,
            &pending.source.entries,
            crate::editor::MAX_PIXELS,
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
                self.message = t!(ctx, "Import cancelled.");
                return;
            }
            widget::Response::Preview(index) => {
                pending.state.prepare(ctx, index, || {
                    registry::admit_import_entry(
                        Frontend::Studio,
                        &pending.source.name,
                        &pending.source.bytes,
                        Platform::Web,
                        index,
                    )
                    .map_err(|e| e.to_string())?;
                    icns::decode_entry(
                        &pending.source.bytes,
                        index,
                        crate::editor::MAX_PIXELS,
                        &|| Ok(()),
                    )
                    .map_err(|e| e.to_string())
                });
            }
            widget::Response::Import(index) => {
                let loaded = (|| {
                    let decoded = pending
                        .state
                        .value()
                        .filter(|decoded| decoded.entry.index == index)
                        .ok_or_else(|| {
                            Error::new(ErrorCode::ConflictStaleBase, "ICNS entry preview changed")
                        })?;
                    Ok::<_, Error>(Loaded {
                        raster_preview: false,
                        converted_copy: true,
                        document: decoded.document(&pending.source.digest)?,
                        name: pending.source.name.clone(),
                        losses: decoded.losses.clone(),
                    })
                })();
                match loaded {
                    Ok(loaded) => {
                        self.pending_import = Some(super::Pending {
                            review: None,
                            loaded,
                            owner: pending.owner,
                        });
                        self.resolve_import(ctx, true);
                        return;
                    }
                    Err(error) => self.message = format!("Open failed: {error}"),
                }
            }
        }
        self.pending_icns = Some(pending);
    }
}
#[cfg(test)]
mod tests;
