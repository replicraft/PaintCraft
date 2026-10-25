//! Browser export options and bounded palette preview ownership.
use super::*;
impl WebApp {
    pub(super) fn export_dialog(&mut self, ctx: &egui::Context) {
        if let Some(mut format) = self.export_confirm {
            egui::Modal::new(egui::Id::unique("web-export-image")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(t!(ui.ctx(), "Export { $format } copy?", &[("format", format.name().into())]));
                egui::ComboBox::from_label(t!(ui.ctx(), "File type")).selected_text(format.name()).show_ui(ui, |ui| {
                    for choice in self.enabled_export_formats() { ui.selectable_value(&mut format, choice, choice.name()); }
                });
                self.export_confirm = Some(format);
                ui.label(t!(ui.ctx(), format.description()));
                let mut preview_ready=true;
                if format==ImageExport::Pcx {
                    paintcraft_ui::pcx_export::controls(ui,&mut self.export_pcx_encoding);
                    let editor=&self.editor;
                    let key=format!("{:?}/{:?}/{:?}/{:?}/{}",editor.session.history_ticket(),editor.tools.preview_revision(),editor.tools.pending_text_action(),self.export_pcx_encoding,self.text.preedit);
                    let encoding=self.export_pcx_encoding;
                    let preedit=self.text.preedit.is_empty();
                    preview_ready=self.pcx_preview.show(ui,key,|| {
                        if !preedit {return Err("text composition pending".into());}
                        let doc=editor.copy_snapshot().map_err(|error|error.to_string())?;
                        let rgba=paintcraft_render::render_flat(&doc).map_err(|error|error.to_string())?;
                        let prepared=paintcraft_formats::pcx::prepare_rgba(doc.canvas.0,doc.canvas.1,&rgba,crate::files::pcx_mode(encoding),&||Ok(())).map_err(|error|error.to_string())?;
                        let preview=prepared.preview(192,&||Ok(())).map_err(|error|error.to_string())?;
                        Ok(paintcraft_ui::pcx_export::Preview{width:preview.width,height:preview.height,rgba:preview.rgba,quantized:prepared.is_quantized()})
                    });
                }else{self.pcx_preview.clear();}
                let (width, height) = self.editor.session.document().canvas;
                ui.label(t!(ui.ctx(), "Image dimensions: { $width } × { $height } px", &[("width", width.to_string()), ("height", height.to_string())]));
                if let Some(label) = format.quality_label() {
                    ui.add(egui::Slider::new(&mut self.export_quality, 1..=100).text(t!(ui.ctx(), label)));
                    ui.label(t!(ui.ctx(), if format == ImageExport::Avif {"AVIF uses lossy compression for colors and transparency, even at quality 100."} else {"Higher quality creates a larger file. Transparent pixels are placed on white."}));
                }
                ui.label(t!(ui.ctx(), "Image exports flatten visible layers and omit hidden layer data. Save a PaintCraft project to preserve editable layers."));
                ui.label(t!(ui.ctx(), "Invisible color information will not be retained."));
                if format == ImageExport::Jpeg { ui.label(t!(ui.ctx(), "JPEG uses lossy compression and cannot preserve transparency.")); }
                ui.horizontal(|ui| {
                    if ui.add_enabled(preview_ready,egui::Button::new(t!(ui.ctx(), "Download { $format }", &[("format", format.name().into())]))).clicked() {
                        self.download_image(ctx, format);
                        self.export_confirm = None;
                    }
                    if ui.button(t!(ui.ctx(), "Cancel")).clicked() { self.export_confirm = None; }
                });
            });
        }
        if self.export_confirm.is_none() {
            self.pcx_preview.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, set_language, translate};
    #[test]
    fn actual_browser_palette_controls_require_a_preview_and_cancel_without_publication() {
        for language in [Language::English, Language::Japanese] {
            let mut h = Harness::builder()
                .with_size(egui::vec2(1360.0, 900.0))
                .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
            let rgba: Vec<u8> = (0..400u32)
                .flat_map(|i| [i as u8, (i >> 8) as u8, i.wrapping_mul(53) as u8, 255])
                .collect();
            let doc = paintcraft_document::import_image(40, 10, &rgba, "browser-pcx-ui").unwrap();
            h.state_mut().editor = Editor::from_document(doc).unwrap();
            h.state_mut().frontend = Frontend::Studio;
            h.state_mut().export_confirm = Some(ImageExport::Pcx);
            set_language(&h.ctx, language);
            h.run();
            let before = h.state().editor.session.snapshot();
            let ticket = h.state().editor.session.ticket();
            let tr = |s| translate(language, s);
            let download = paintcraft_ui::i18n::format_message(
                language,
                "Download { $format }",
                &[("format", "PCX".into())],
            );
            h.get_by_label(&tr("PCX encoding")).click();
            h.run();
            h.get_by_label(&tr("Indexed: exact colors")).click();
            h.run();
            assert!(h.get_by_label(&download).accesskit_node().is_disabled());
            assert!(
                h.query_by_label(&tr(
                    "PCX preview is unavailable. Reduce the image size or choose RGB24."
                ))
                .is_some()
            );
            h.get_by_label(&tr("PCX encoding")).click();
            h.run();
            h.get_by_label(&tr("Indexed: up to 256 colors")).click();
            h.run();
            assert!(h.query_by_label(&tr("Export preview")).is_some());
            assert!(
                h.query_by_label(&tr(
                    "Palette conversion changes colors. Exporting accepts this color loss."
                ))
                .is_some()
            );
            assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
            h.get_by_label(&tr("Cancel")).click();
            h.run();
            assert!(h.state().export_confirm.is_none());
            assert!(std::sync::Arc::ptr_eq(
                &before,
                &h.state().editor.session.snapshot()
            ));
            assert_eq!(h.state().editor.session.ticket(), ticket);
        }
    }
}
