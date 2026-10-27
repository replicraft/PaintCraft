use super::*;
impl App {
    pub(crate) fn msp_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.msp_export.take() else {
            self.confirm = None;
            return;
        };
        if !state.current(self) {
            self.msp_error(stale());
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("msp-export-review")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading(t!(ctx, "MSP picture"));
            let mut changed = false;
            if state.destination.is_none() {
                changed = controls::controls(ui, &mut state.version);
            }
            if let Some(texture) = &state.texture {
                ui.label(t!(ctx, "Export preview"));
                ui.image(texture);
                state.shown.get_or_insert(ctx.cumulative_frame_nr());
            } else if let Some(error) = &state.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            } else {
                ui.spinner();
                ui.label(t!(ctx, "Preparing MSP picture…"));
            }
            if let Some(prepared) = &state.prepared {
                for loss in prepared.artifact.losses() {
                    ui.label(t!(
                        ctx,
                        match loss.as_str() {
                            "flatten" => "Visible layers will be flattened into one picture.",
                            "hidden-rgb" => "Invisible color information will not be retained.",
                            _ => loss.as_str(),
                        }
                    ));
                }
            }
            if let Some(prepared) = &state.prepared {
                ui.label(t!(
                    ctx,
                    "Encoded size: { $bytes } bytes",
                    &[("bytes", prepared.artifact.bytes().len().to_string())]
                ));
                if !prepared.artifact.losses().is_empty() {
                    ui.checkbox(
                        &mut state.losses_accepted,
                        t!(ctx, "I accept these export changes."),
                    );
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
                && state
                    .prepared
                    .as_ref()
                    .is_some_and(|p| p.artifact.losses().is_empty() || state.losses_accepted)
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
            self.msp_export = Some(state);
            return;
        }
        let (changed, cancel, accept) = response.inner;
        if cancel || response.should_close() {
            self.confirm = None;
            return;
        }
        if changed {
            state.retire();
            state.error = None;
            ctx.request_repaint();
        }
        let publish = state.destination.is_some();
        self.msp_export = Some(state);
        if accept {
            if publish {
                self.finish_msp_export();
            } else {
                self.choose_msp_destination(ctx);
            }
        }
    }
}
