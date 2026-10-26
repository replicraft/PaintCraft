use super::*;

impl Source {
    pub(super) fn sync_fonts(&mut self, pick: Option<&FontPick>) {
        self.state.set_font_files(
            svg_import::FontKey {
                source: self.id.as_u128(),
                files: self.font_files.as_u128(),
            },
            self.additional
                .iter()
                .map(|font| {
                    font.source
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            pick.is_some_and(|pick| pick.source == self.id && pick.files == self.font_files),
        );
    }
    pub(super) fn remove_font(&mut self, index: usize) {
        if index < self.additional.len() {
            self.additional.remove(index);
            self.font_files = Uuid::new_v4();
            self.fonts = None;
            self.sync_fonts(None);
        }
    }
    pub(super) fn build_fonts(&self, extra: Option<&[u8]>) -> Result<svg::Fonts> {
        let mut fonts = paintcraft_ui::fonts::bundled_data().to_vec();
        fonts.extend(self.additional.iter().map(|font| font.bytes.as_slice()));
        fonts.extend(extra);
        svg::Fonts::from_data(&fonts, "Selawik", &|| Ok(()))
    }
    fn remaining_bytes(&self) -> Result<usize> {
        let bundled = paintcraft_ui::fonts::bundled_data();
        if bundled.len() + self.additional.len() >= svg::FONT_FILES_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "SVG font inventory exceeds allowance",
            ));
        }
        let bytes = bundled.iter().map(|font| font.len()).sum::<usize>()
            + self
                .additional
                .iter()
                .map(|font| font.bytes.len())
                .sum::<usize>();
        svg::FONT_BYTES_MAX
            .checked_sub(bytes)
            .filter(|remaining| *remaining > 0)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "SVG font bytes exceed allowance"))
    }
}
impl App {
    pub(super) fn request_svg_font_picker(
        &mut self,
        ctx: &egui::Context,
        source: Uuid,
        files: Uuid,
    ) -> Result<()> {
        let dialogs = self
            .dialogs
            .as_mut()
            .filter(|dialogs| !dialogs.pending())
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Native font file picker is unavailable",
                )
            })?;
        self.svg_font_picker = Some(FontPick { source, files });
        let repaint = ctx.clone();
        dialogs.request_with_filters(
            Kind::ImportSvgFont,
            None,
            &[paintcraft_platform::dialogs::Filter {
                name: "Font files".into(),
                extensions: ["ttf", "otf", "ttc", "otc"].map(str::to_owned).into(),
            }],
            |source| t!(ctx, source),
            move || repaint.request_repaint(),
        );
        Ok(())
    }
    pub(crate) fn complete_svg_font_picker(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        let Some(pick) = self.svg_font_picker.take() else {
            return;
        };
        if !self.pending_import.as_ref().is_some_and(|pending| pending.current(self)
            && matches!(&pending.candidate, Candidate::Svg(source) if source.id == pick.source && source.font_files == pick.files)) {
            self.editor.set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Some(path) = path else {
            return;
        };
        let mut pending = self.pending_import.take().unwrap();
        let Candidate::Svg(source) = &mut pending.candidate else {
            unreachable!();
        };
        let result = (|| {
            if path.file_name().unwrap_or_default().to_string_lossy().len() > 4096 {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "SVG font filename exceeds allowance",
                ));
            }
            let allowance = source.remaining_bytes()?;
            self.guard_asset_adoption(&path, None)?;
            let acquired =
                crate::format_admission::acquire_resource_source(&path, allowance as u64)?;
            self.guard_asset_adoption(&path, Some(&acquired.identity))?;
            let fonts = source.build_fonts(Some(&acquired.bytes))?;
            source.additional.push(FontFile {
                bytes: acquired.bytes,
                source: crate::destination::SourceGuard {
                    path,
                    identity: acquired.identity,
                },
            });
            source.fonts = Some(fonts);
            source.font_files = Uuid::new_v4();
            source.sync_fonts(None);
            Ok::<_, Error>(())
        })();
        if let Err(error) = result {
            source.state.font_failure(t!(
                ctx,
                "Could not add font files: { $detail }",
                detail = error
            ));
        }
        self.pending_import = Some(pending);
    }
}
#[cfg(test)]
#[path = "fonts/tests.rs"]
mod tests;
