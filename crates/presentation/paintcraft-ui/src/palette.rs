//! Palette-resource presentation. Hosts own precise assets, byte acquisition and I/O.
use crate::t;

#[derive(Clone, Debug)]
pub struct Swatch {
    pub name: String,
    pub model: &'static str,
    pub kind: &'static str,
    pub components: String,
    pub rgba: Option<[u8; 4]>,
}
#[derive(Clone, Debug)]
pub enum Row {
    Group(String),
    Swatch(Swatch),
}
#[derive(Clone, Debug, Default)]
pub struct Resource {
    pub name: String,
    pub rows: Vec<Row>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Close,
    Import,
    Export,
    Pick([u8; 4]),
}

pub fn dialog(
    ctx: &egui::Context,
    resource: Option<&Resource>,
    busy: bool,
    error: Option<&str>,
) -> Response {
    let mut response = Response::Pending;
    egui::Modal::new(egui::Id::unique("studio-palette-assets")).show(ctx, |ui| {
        ui.set_width(520.0_f32.min((ctx.viewport_rect().width() - 48.0).max(220.0)));
        ui.heading(t!(ctx, "Palettes"));
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t!(ctx, "Import palette…")).clicked() {
                    response = Response::Import;
                }
                if ui.add_enabled(resource.is_some(), egui::Button::new(t!(ctx, "Export palette…"))).clicked() {
                    response = Response::Export;
                }
            });
        });
        if busy { ui.spinner(); }
        if let Some(error) = error { ui.colored_label(ui.visuals().error_fg_color, error); }
        ui.label(t!(ctx, "Picking a swatch copies an 8-bit sRGB color. Original palette values stay unchanged."));
        ui.label(t!(ctx, "CMYK, LAB and out-of-range colors need color-managed conversion and cannot be picked yet."));
        if let Some(resource) = resource {
            ui.label(&resource.name);
            egui::ScrollArea::vertical()
                .id_salt("palette-resource-rows")
                .max_height((ctx.viewport_rect().height() - 280.0).clamp(90.0, 420.0))
                .show_rows(ui, 48.0, resource.rows.len(), |ui, range| {
                    for index in range {
                        ui.push_id(index, |ui| {
                            ui.set_min_height(48.0);
                            match &resource.rows[index] {
                                Row::Group(name) => { ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate()); }
                                Row::Swatch(swatch) => {
                                    if let Some(rgba) = swatch_row(ui, swatch, !busy) {
                                        response = Response::Pick(rgba);
                                    }
                                }
                            }
                        });
                    }
                });
        } else {
            ui.label(t!(ctx, "Import an Adobe Swatch Exchange palette to browse its colors and groups."));
        }
        ui.separator();
        if ui.add_enabled(!busy, egui::Button::new(t!(ctx, "Close"))).clicked() {
            response = Response::Close;
        }
    });
    if !busy && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        response = Response::Close;
    }
    response
}
fn swatch_row(ui: &mut egui::Ui, swatch: &Swatch, enabled: bool) -> Option<[u8; 4]> {
    let mut picked = None;
    ui.horizontal(|ui| {
        let title = if swatch.name.is_empty() {
            t!(ui.ctx(), "Unnamed swatch")
        } else {
            swatch.name.clone()
        };
        let button = egui::Button::new(" ").min_size(egui::vec2(32.0, 32.0));
        let button = if let Some(rgba) = swatch.rgba {
            button.fill(egui::Color32::from_rgba_unmultiplied(
                rgba[0], rgba[1], rgba[2], rgba[3],
            ))
        } else {
            button
        };
        let enabled = enabled && swatch.rgba.is_some();
        let response = ui.add_enabled(enabled, button).on_hover_text(&title);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &title));
        if response.clicked() {
            picked = swatch.rgba;
        }
        ui.vertical(|ui| {
            ui.add(egui::Label::new(&title).truncate())
                .on_hover_text(&title);
            let detail = format!(
                "{} · {} · {}",
                swatch.model,
                t!(ui.ctx(), swatch.kind),
                swatch.components
            );
            ui.add(egui::Label::new(egui::RichText::new(&detail).small()).truncate())
                .on_hover_text(detail);
        });
    });
    picked
}

#[cfg(test)]
mod tests {
    #[test]
    fn palette_controls_and_adoption_policy_are_translated() {
        for language in crate::i18n::Language::ALL
            .into_iter()
            .filter(|lang| *lang != crate::i18n::Language::English)
        {
            for source in [
                "Palettes",
                "Palettes…",
                "Import palette…",
                "Export palette…",
                "Global color",
                "Spot color",
                "Process color",
                "Unnamed swatch",
                "Picking a swatch copies an 8-bit sRGB color. Original palette values stay unchanged.",
                "CMYK, LAB and out-of-range colors need color-managed conversion and cannot be picked yet.",
                "Import an Adobe Swatch Exchange palette to browse its colors and groups.",
            ] {
                assert_ne!(
                    crate::i18n::translate(language, source),
                    source,
                    "{}: {source}",
                    language.id()
                );
            }
        }
    }
    #[test]
    fn resource_action_is_studio_only_with_its_own_identity() {
        use crate::actions::{Action, Frontend};
        assert!(Action::Palettes.available(Frontend::Studio));
        assert!(!Action::Palettes.available(Frontend::Classic));
        assert_ne!(
            Action::Palettes.activation_code(),
            Action::ExportIcons.activation_code()
        );
    }
}
