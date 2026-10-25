//! Shared explicit palette choices; hosts own raster preparation and admission.
use crate::t;
use egui::{ColorImage, TextureHandle, TextureOptions, Ui};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Rgb24,
    IndexedExact,
    Indexed256,
}
pub fn controls(ui: &mut Ui, encoding: &mut Encoding) {
    egui::ComboBox::from_label(t!(ui.ctx(), "PCX encoding"))
        .selected_text(t!(ui.ctx(), label(*encoding)))
        .show_ui(ui, |ui| {
            for mode in [
                Encoding::Rgb24,
                Encoding::IndexedExact,
                Encoding::Indexed256,
            ] {
                ui.selectable_value(encoding, mode, t!(ui.ctx(), label(mode)));
            }
        });
    ui.label(t!(
        ui.ctx(),
        match encoding {
            Encoding::Rgb24 => "Preserve full RGB colors. Transparency is placed on white.",
            Encoding::IndexedExact =>
                "Keep exact colors in a palette. Images needing more than 256 colors are refused.",
            Encoding::Indexed256 =>
                "Convert to at most 256 colors when needed. Review the preview before exporting.",
        }
    ));
}
fn label(mode: Encoding) -> &'static str {
    match mode {
        Encoding::Rgb24 => "RGB24",
        Encoding::IndexedExact => "Indexed: exact colors",
        Encoding::Indexed256 => "Indexed: up to 256 colors",
    }
}

#[derive(Debug)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub quantized: bool,
}
pub struct Cache<K> {
    key: Option<K>,
    image: Option<TextureHandle>,
    quantized: bool,
    error: bool,
    prepared_frame: Option<u64>,
}
impl<K> Default for Cache<K> {
    fn default() -> Self {
        Self {
            key: None,
            image: None,
            quantized: false,
            error: false,
            prepared_frame: None,
        }
    }
}
impl<K> std::fmt::Debug for Cache<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcxPreviewCache")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl<K: PartialEq> Cache<K> {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn show(
        &mut self,
        ui: &mut Ui,
        key: K,
        prepare: impl FnOnce() -> Result<Preview, String>,
    ) -> bool {
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.image = None;
            self.error = false;
            self.prepared_frame = Some(ui.ctx().cumulative_frame_nr());
            ui.ctx().request_repaint();
            match prepare() {
                Ok(preview)
                    if u64::from(preview.width) * u64::from(preview.height) * 4
                        == preview.rgba.len() as u64
                        && preview.width > 0
                        && preview.height > 0
                        && preview.width <= 256
                        && preview.height <= 256 =>
                {
                    self.quantized = preview.quantized;
                    self.image = Some(ui.ctx().load_texture(
                        "pcx-export-preview",
                        ColorImage::from_rgba_unmultiplied(
                            [preview.width as usize, preview.height as usize],
                            &preview.rgba,
                        ),
                        TextureOptions::NEAREST,
                    ));
                }
                _ => self.error = true,
            }
        }
        if self.error {
            ui.colored_label(
                ui.visuals().error_fg_color,
                t!(
                    ui.ctx(),
                    "PCX preview is unavailable. Reduce the image size or choose RGB24."
                ),
            );
            return false;
        }
        ui.label(t!(ui.ctx(), "Export preview"));
        if let Some(image) = &self.image {
            ui.image(image);
        }
        if self.quantized {
            ui.label(t!(
                ui.ctx(),
                "Palette conversion changes colors. Exporting accepts this color loss."
            ));
        }
        self.prepared_frame != Some(ui.ctx().cumulative_frame_nr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_controls_and_preview_warnings_are_translated_in_every_language() {
        for language in crate::i18n::Language::ALL {
            if language == crate::i18n::Language::English {
                continue;
            }
            for text in [
                "PCX encoding",
                label(Encoding::IndexedExact),
                label(Encoding::Indexed256),
                "Preserve full RGB colors. Transparency is placed on white.",
                "Keep exact colors in a palette. Images needing more than 256 colors are refused.",
                "Convert to at most 256 colors when needed. Review the preview before exporting.",
                "PCX preview is unavailable. Reduce the image size or choose RGB24.",
                "Export preview",
                "Palette conversion changes colors. Exporting accepts this color loss.",
            ] {
                assert_ne!(
                    crate::i18n::translate(language, text),
                    text,
                    "{}: {text}",
                    language.id()
                );
            }
        }
    }
    #[test]
    fn changed_source_must_be_displayed_before_export_and_stable_source_is_not_rebuilt() {
        let ctx = egui::Context::default();
        let mut cache = Cache::default();
        let mut builds = 0;
        for (key, expected) in [(1, false), (1, true), (2, false), (2, true)] {
            let mut states = Vec::new();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                states.push(cache.show(ui, key, || {
                    builds += 1;
                    Ok(Preview {
                        width: 1,
                        height: 1,
                        rgba: vec![17, 51, 93, 255],
                        quantized: false,
                    })
                }));
            });
            output.textures_delta.clear();
            assert!(!states.is_empty());
            assert!(states.iter().all(|ready| *ready == expected));
        }
        assert_eq!(builds, 2);
    }
}
