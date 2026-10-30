use super::*;
impl WebApp {
    pub(crate) fn studio_filter_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.filter_editing.take() else {
            return;
        };
        let before = state.draft.parameters;
        let mask = if let paintcraft_commands::FilterOp::Update {
            filter: paintcraft_commands::FilterRef::Id(id),
            ..
        } = &state.draft.base.operation
        {
            state
                .draft
                .source
                .layer(&state.draft.layer)
                .and_then(|l| l.filters.iter().find(|f| f.id() == *id))
                .and_then(|f| f.mask())
                .map(|m| {
                    let stats = m.coverage().stats();
                    filters::MaskSummary {
                        enabled: m.enabled(),
                        runs: stats.runs,
                    }
                })
        } else {
            None
        };
        let response = filters::parameter_dialog(ctx, &mut state.draft.parameters, mask);
        if before != state.draft.parameters {
            match state.draft.change() {
                Ok(()) => {
                    self.canvas.texture = state.base_texture.clone();
                    self.canvas.filter_footprint = state.base_footprint;
                    self.canvas.invalidate();
                }
                Err(error) => state.draft.error = Some(error),
            }
        }
        match response {
            ParameterResponse::Apply(_) => {
                state.draft.apply = Some(state.draft.revision);
                state.draft.error = None;
                self.filter_display.renderer.retry();
            }
            ParameterResponse::Cancel => {
                self.filter_editing = Some(state);
                self.cancel_filter_edit();
                return;
            }
            ParameterResponse::Preview(_) | ParameterResponse::Pending => {}
        }
        if let Some(error) = &state.draft.error {
            egui::Window::new(t!(ctx, "Preview unavailable"))
                .id(egui::Id::unique("web-filter-error"))
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(paintcraft_ui::i18n::tr_status(
                        ctx,
                        &format!("Filter not applied: {error}"),
                    ));
                });
        }
        self.filter_editing = Some(state);
        self.queue_filter_preview();
        self.finish_filter_apply(ctx);
    }
}
