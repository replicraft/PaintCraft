use super::*;
impl App {
    pub(crate) fn dds_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.dds_export.take() else {
            self.confirm = None;
            return;
        };
        if !state.current(self) {
            self.dds_error(stale());
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("dds-export-review")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading(t!(ctx, "DDS texture"));
            let mut changed = false;
            if state.destination.is_none() {
                changed = controls::controls(
                    ui,
                    &mut state.settings,
                    self.editor.document().canvas.0,
                    self.editor.document().canvas.1,
                );
            }
            if let Some(texture) = &state.texture {
                ui.label(t!(ctx, "Base image preview"));
                ui.image(texture);
                state.shown.get_or_insert(ctx.cumulative_frame_nr());
            } else if let Some(error) = &state.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            } else {
                ui.spinner();
                ui.label(t!(ctx, "Preparing DDS texture…"));
            }
            if let Some(prepared) = &state.prepared {
                for loss in prepared.artifact.losses() {
                    ui.label(t!(
                        ctx,
                        match loss.as_str() {
                            "flatten" => "Visible layers will be flattened into one picture.",
                            "hidden-rgb" => "Invisible color information will not be retained.",
                            dds::BLOCK_COMPRESSION_LOSS =>
                                "Block compression can change colors and transparency.",
                            dds::MIP_HIDDEN_RGB_LOSS =>
                                "Mip filtering discards invisible colors in smaller images.",
                            export::dds::LINEAR_QUANTIZATION_LOSS =>
                                "Converting to linear 8-bit samples loses color precision.",
                            _ => loss.as_str(),
                        }
                    ));
                }
            }
            if let Some((path, expected)) = &state.destination {
                ui.label(path.display().to_string());
                if expected.is_some() {
                    ui.label(t!(ctx, "The file exists and will be replaced."));
                }
            }
            let ready = !changed
                && state.texture.is_some()
                && state.prepared.is_some()
                && state
                    .shown
                    .is_some_and(|frame| frame < ctx.cumulative_frame_nr());
            ui.horizontal(|ui| {
                let cancel = ui.button(t!(ctx, "Cancel")).clicked();
                let accept = ui
                    .add_enabled(
                        ready,
                        egui::Button::new(t!(
                            ctx,
                            if state.destination.is_some() {
                                "Export now"
                            } else {
                                "Choose destination…"
                            }
                        )),
                    )
                    .clicked();
                (changed, cancel, accept)
            })
            .inner
        });
        if ctx.current_pass_index() != 0 {
            self.dds_export = Some(state);
            return;
        }
        let (changed, cancel, accept) = response.inner;
        if cancel || response.should_close() {
            self.confirm = None;
            return;
        }
        if changed {
            match effective(state.settings, self.editor.document().canvas) {
                Ok(settings) => {
                    state.settings = settings;
                    state.retire();
                    state.error = None;
                    if settings == Settings::default() {
                        self.confirm = None;
                        self.begin_export_options(paintcraft_ui::actions::ImageExport::Dds);
                        return;
                    }
                }
                Err(error) => {
                    state.retire();
                    state.error = Some(error.to_string());
                }
            }
            ctx.request_repaint();
        }
        let publish = state.destination.is_some();
        self.dds_export = Some(state);
        if accept {
            if publish {
                self.finish_dds_export();
            } else {
                self.choose_dds_destination(ctx);
            }
        }
    }
}
