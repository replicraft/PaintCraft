//! Browser export options and bounded palette preview ownership.
use super::*;
mod frozen;
pub(crate) use frozen::Frozen;
#[cfg(test)]
mod matte_tests;
#[cfg(test)]
mod metadata_tests;
#[cfg(test)]
mod persistence_tests;
impl WebApp {
    pub(super) fn export_dialog(&mut self, ctx: &egui::Context) {
        if self.dds.reviewing() {
            self.dds_export_dialog(ctx);
            return;
        }
        if self.msp.reviewing() {
            self.msp_export_dialog(ctx);
            return;
        }
        if let Some(mut format) = self.export_confirm {
            if self.export_frozen.is_none() {
                match Frozen::capture(self) {
                    Ok(frozen) => self.export_frozen = Some(frozen),
                    Err(error) => {
                        self.message = error.to_string();
                        self.export_confirm = None;
                        return;
                    }
                }
            }
            if !self
                .export_frozen
                .as_ref()
                .is_some_and(|frozen| frozen.current(self))
            {
                self.export_confirm = None;
                self.export_frozen = None;
                self.export_omit_source_metadata = false;
                self.export_bake_filters = false;
                self.message = t!(ctx, "The document changed; choose the image again.");
                return;
            }
            egui::Modal::new(egui::Id::unique("web-export-image")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(t!(ui.ctx(), "Export { $format } copy?", &[("format", format.name().into())]));
                egui::ComboBox::from_label(t!(ui.ctx(), "File type")).selected_text(format.name()).show_ui(ui, |ui| {
                    for choice in self.enabled_export_formats() { ui.selectable_value(&mut format, choice, choice.name()); }
                });
                self.export_confirm = Some(format);
                ui.label(t!(ui.ctx(), format.description()));
                let advanced_dds=format==ImageExport::Dds && self.frontend==Frontend::Studio && dds_exporting::admit().is_ok();
                if advanced_dds {paintcraft_ui::dds_export::controls(ui,&mut self.dds_settings,self.editor.session.document().canvas.0,self.editor.session.document().canvas.1);}
                let advanced_msp=format==ImageExport::Msp && self.frontend==Frontend::Studio && msp_exporting::admit().is_ok();
                if advanced_msp {paintcraft_ui::msp_export::controls(ui,&mut self.msp_version);}
                let mut preview_ready=true;
                if format==ImageExport::Pcx {
                    paintcraft_ui::pcx_export::controls(ui,&mut self.export_pcx_encoding);
                    let editor=&self.editor;
                    let snapshot=self.export_frozen.as_ref().expect("frozen export").document.clone();
                    let key=format!("{:?}/{:?}/{:?}/{:?}/{}",editor.session.history_ticket(),editor.tools.preview_revision(),editor.tools.pending_text_action(),self.export_pcx_encoding,self.text.preedit());
                    let encoding=self.export_pcx_encoding;
                    let preedit=!self.text.is_composing();
                    preview_ready=self.pcx_preview.show(ui,key,|| {
                        if !preedit {return Err("text composition pending".into());}
                        let doc=snapshot;
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
                    ui.label(t!(ui.ctx(), if format == ImageExport::Avif {"AVIF uses lossy compression for colors and transparency, even at quality 100."} else {"Higher quality creates a larger file."}));
                }
                if format == ImageExport::Jpeg { paintcraft_ui::jpeg_export::controls(ui, &mut self.export_jpeg_matte); }
                ui.label(t!(ui.ctx(), "Image exports flatten visible layers and omit hidden layer data. Save a PaintCraft project to preserve editable layers."));
                ui.label(t!(ui.ctx(), "Invisible color information will not be retained."));
                if format == ImageExport::Jpeg { ui.label(t!(ui.ctx(), "JPEG uses lossy compression and cannot preserve transparency.")); }
                let advanced=advanced_dds&&dds_exporting::advanced(self.dds_settings,self.editor.session.document().canvas);
                let msp2=advanced_msp&&self.msp_version==paintcraft_ui::msp_export::Version::RowRleV2;
                if !advanced && !msp2 && self.export_frozen.as_ref().is_some_and(|frozen| frozen.document.source_metadata.is_some()) {
                    paintcraft_ui::source_metadata_export::controls(ui, &mut self.export_omit_source_metadata);
                    preview_ready &= self.export_omit_source_metadata;
                }
                if !advanced && !msp2 && self.export_frozen.as_ref().is_some_and(|f| f.has_filters) {
                    paintcraft_ui::filter_bake_export::controls(ui, &mut self.export_bake_filters);
                    preview_ready &= self.export_bake_filters;
                }
                ui.horizontal(|ui| {
                    let caption=if advanced||msp2{t!(ui.ctx(),"Preview export")}else{t!(ui.ctx(),"Download { $format }", &[("format", format.name().into())])};
                    if ui.add_enabled(preview_ready,egui::Button::new(caption)).clicked() {
                        if advanced {self.begin_dds_export(ctx);} else if msp2 {self.begin_msp_export(ctx);} else {self.download_image(ctx, format);self.export_confirm=None;}
                    }
                    if ui.button(t!(ui.ctx(), "Cancel")).clicked() { self.export_confirm = None; }
                });
            });
        }
        if self.export_confirm.is_none() {
            self.pcx_preview.clear();
            self.export_frozen = None;
            self.export_omit_source_metadata = false;
            self.export_bake_filters = false;
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

#[cfg(test)]
#[path = "exporting/filter_bake_tests.rs"]
mod filter_bake_tests;
