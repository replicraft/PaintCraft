//! Browser settings use the same presentation as desktop with staged adoption.
use super::*;
use crate::preferences::Preferences;
use paintcraft_classic::settings::{Page, Response, card};
use paintcraft_ui::i18n::picker;

#[derive(Debug)]
pub(super) struct Draft {
    values: Preferences,
    page: Page,
    search: String,
    original_theme: egui::ThemePreference,
}
impl WebApp {
    pub(super) fn refresh_appearance(&mut self, ctx: &egui::Context) {
        let values = self
            .settings_draft
            .as_ref()
            .map_or(&self.preferences, |draft| &draft.values);
        let variant = self
            .settings_draft
            .as_ref()
            .map_or(self.variant, |draft| draft.values.variant);
        let preferred = values
            .theme
            .unwrap_or_else(|| ctx.system_theme().unwrap_or(egui::Theme::Light));
        let theme = if self.frontend == Frontend::Classic {
            variant.effective_theme(preferred)
        } else {
            preferred
        };
        ctx.set_theme(theme);
        let key = (theme, variant, self.frontend);
        if self.applied_style != Some(key) {
            self.applied_style = Some(key);
            paintcraft_ui::theme::apply(ctx, key.0);
            if self.frontend == Frontend::Studio {
                paintcraft_studio::apply_style_for_theme(ctx, key.0);
            } else {
                paintcraft_classic::apply_style_for_theme(ctx, key.1, key.0);
            }
            ctx.request_repaint();
        }
    }
    pub(super) fn open_settings(&mut self, ctx: &egui::Context) {
        if self.paste.busy() {
            self.cancel_paste(ctx);
        }
        self.settings = true;
        self.settings_draft = Some(Draft {
            values: self.preferences.clone(),
            page: Page::Appearance,
            search: String::new(),
            original_theme: ctx.options(|options| options.theme_preference),
        });
    }
    pub(super) fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings {
            return;
        }
        let Some(mut draft) = self.settings_draft.take() else {
            self.open_settings(ctx);
            return;
        };
        let mut page = draft.page;
        let response =
            paintcraft_classic::settings::show(ctx, true, &mut page, |ui, page, width, height| {
                content(
                    ui,
                    &mut draft,
                    &self.text.families,
                    page,
                    width,
                    height,
                    self.frontend == Frontend::Classic,
                )
            });
        draft.page = page;
        match response {
            Response::Cancel => {
                self.settings = false;
                ctx.set_theme(draft.original_theme);
                set_language(ctx, self.preferences.language);
                self.refresh_appearance(ctx);
            }
            Response::Apply if draft.values.validate().is_ok() => {
                self.settings = false;
                self.preferences = draft.values;
                set_language(ctx, self.preferences.language);
                ctx.set_theme(
                    self.preferences
                        .theme
                        .map_or(egui::ThemePreference::System, Into::into),
                );
                self.variant = self.preferences.variant;
                self.refresh_appearance(ctx);
                self.text
                    .style
                    .family
                    .clone_from(&self.preferences.default_font);
                self.sync_text();
                self.persist_preferences();
            }
            Response::Apply | Response::Pending => {
                self.settings_draft = Some(draft);
            }
        }
        if response != Response::Pending {
            ctx.request_repaint();
        }
        if let Some(draft) = &self.settings_draft {
            set_language(ctx, draft.values.language);
            ctx.set_theme(
                draft
                    .values
                    .theme
                    .map_or(egui::ThemePreference::System, Into::into),
            );
            self.refresh_appearance(ctx);
        }
    }
}
fn content(
    ui: &mut egui::Ui,
    draft: &mut Draft,
    fonts: &[String],
    page: Page,
    width: f32,
    height: f32,
    classic: bool,
) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, height), egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            paintcraft_classic::form_control_style(ui);
            egui::ScrollArea::vertical()
                .id_salt(("settings-page", page))
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| match page {
                    Page::Appearance | Page::Interface => appearance(ui, &mut draft.values, classic),
                    Page::Canvas => canvas(ui, &mut draft.values),
                    Page::Text => text(ui, draft, fonts),
                    Page::About => card(ui, "About", |ui| {
                        paintcraft_classic::settings::about(ui);
                        ui.weak(t!(
                            ui.ctx(),
                            "Preferences are stored in this browser when storage is available. Document pixels are not stored."
                        ));
                    }),
                });
        },
    );
}
fn text(ui: &mut egui::Ui, draft: &mut Draft, fonts: &[String]) {
    card(ui, "Text editing", |ui| {
        ui.label(t!(ui.ctx(), "Default font"));
        paintcraft_classic::font_family(
            ui,
            &mut draft.values.default_font,
            fonts,
            &mut draft.search,
            false,
        );
        ui.label(t!(
            ui.ctx(),
            "{ $count } available font families",
            &[("count", fonts.len().to_string())]
        ));
        ui.checkbox(
            &mut draft.values.editable_until_save,
            t!(ui.ctx(), "Keep text editable until save (prototype)"),
        );
        ui.weak(t!(
            ui.ctx(),
            "Prototype preference only. Text is rasterized when placed; editable text objects and save-time rasterization are not available yet."
        ));
    });
}

fn appearance(ui: &mut egui::Ui, values: &mut Preferences, classic: bool) {
    card(ui, "App language", |ui| {
        picker(ui, &mut values.language);
    });
    card(ui, "App theme", |ui| {
        ui.weak(t!(
            ui.ctx(),
            "Preview the theme now. Apply saves your choice; Cancel restores it."
        ));
        if classic && values.variant == ClassicVariant::V7 {
            ui.label(t!(
                ui.ctx(),
                "Paint 7 uses light mode. Your preferred theme returns with other skins."
            ));
            ui.add_enabled(false, egui::RadioButton::new(true, t!(ui.ctx(), "Light")));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (theme, label) in [
                (None, "Use system setting"),
                (Some(egui::Theme::Light), "Light"),
                (Some(egui::Theme::Dark), "Dark"),
            ] {
                ui.radio_value(&mut values.theme, theme, t!(ui.ctx(), label));
            }
        });
    });
    card(ui, "Classic skin", |ui| {
        ui.horizontal_wrapped(|ui| {
            for variant in ClassicVariant::ALL {
                ui.radio_value(&mut values.variant, variant, variant.name());
            }
        });
    });
}
fn canvas(ui: &mut egui::Ui, values: &mut Preferences) {
    card(ui, "New Classic canvas", |ui| {
        ui.weak(t!(
            ui.ctx(),
            "These defaults apply to new documents, leaving the current image unchanged."
        ));
        paintcraft_classic::canvas_size::dimension_fields(
            ui,
            &mut values.canvas.configured,
            paintcraft_document::CANVAS_SIDE_MAX,
            ["Default width", "Default height"],
        );
        let text = t!(
            ui.ctx(),
            "Image limit: { $maximum } pixels",
            &[("maximum", crate::editor::MAX_PIXELS.to_string())]
        );
        if values.validate().is_err() {
            ui.colored_label(ui.visuals().error_fg_color, text);
        } else {
            ui.weak(text);
        }
    });
    card(ui, "Canvas size on startup", |ui| {
        ui.radio_value(
            &mut values.canvas.remember_last,
            true,
            t!(ui.ctx(), "Remember last canvas size"),
        );
        ui.radio_value(
            &mut values.canvas.remember_last,
            false,
            t!(ui.ctx(), "Use configured default size"),
        );
        if let Some((w, h)) = values.canvas.last {
            ui.weak(t!(
                ui.ctx(),
                "Last canvas: { $width } × { $height } pixels",
                &[("width", w.to_string()), ("height", h.to_string())]
            ));
        }
    });
}

#[cfg(test)]
mod tests;
