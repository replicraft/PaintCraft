//! Explicit SVG conversion keeps bytes, preview and publication owner together.
use super::*;
use paintcraft_formats::{
    registry::{self, Platform},
    svg,
};
use paintcraft_ui::svg_import::{self, Response};

pub(super) struct Source {
    bytes: Vec<u8>,
    fonts: Option<svg::Fonts>,
    state: svg_import::State<paintcraft_formats::project_import::Converted>,
}
impl App {
    pub(crate) fn stage_svg_import(&mut self, bytes: Vec<u8>, path: PathBuf) -> Result<()> {
        registry::admit_import_svg(
            self.format_frontend(),
            &path.to_string_lossy(),
            &bytes,
            Platform::Native,
        )?;
        let digest = hex(&Sha256::digest(&bytes));
        self.stage_candidate(
            Candidate::Svg(Source {
                bytes,
                fonts: None,
                state: Default::default(),
            }),
            path,
            digest,
            true,
        )
    }
    pub(super) fn svg_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Svg(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let name = pending
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        match source.state.dialog(ctx, &name) {
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
                    options.admit(image::DECODE_PIXELS_MAX)?;
                    if source.fonts.is_none() {
                        source.fonts = Some(svg::Fonts::from_data(
                            &paintcraft_ui::fonts::bundled_data(),
                            "Selawik",
                            &|| Ok(()),
                        )?);
                    }
                    let raster = svg::prepare(
                        &source.bytes,
                        options,
                        source.fonts.as_ref().unwrap(),
                        image::DECODE_PIXELS_MAX,
                        &|| Ok(()),
                    )?
                    .render(&|| Ok(()))?;
                    let preview = raster.preview(&|| Ok(()))?;
                    let document = raster.document(&pending.digest, &|| Ok(()))?;
                    let losses = raster.losses;
                    let converted = paintcraft_formats::project_import::Converted {
                        document,
                        losses: losses.clone(),
                    };
                    if !source.state.install(
                        ctx,
                        config,
                        converted,
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
                    source.state.fail(error.to_string());
                }
            }
            Response::Import => {
                if let Some(converted) = source.state.take(ctx) {
                    pending.candidate = Candidate::Layered(converted);
                    self.pending_import = Some(pending);
                    self.resolve_import(ctx, true);
                    return;
                }
            }
            Response::Cancel => {
                self.pending_import = Some(pending);
                self.resolve_import(ctx, false);
                return;
            }
            Response::Pending => {}
        }
        self.pending_import = Some(pending);
    }
}

#[cfg(test)]
#[path = "svg/tests.rs"]
mod tests;
