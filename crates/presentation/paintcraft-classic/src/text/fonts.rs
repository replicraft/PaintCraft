//! Search stays fixed while the family list scrolls. Only visible rows are drawn.
use super::*;

pub(super) fn family(
    ui: &mut Ui,
    family: &mut String,
    families: &[String],
    search: &mut String,
    loading: bool,
) {
    let popup_height = (ui.ctx().content_rect().height() * 0.5 - 24.0).clamp(120.0, 340.0);
    let mut search_response = None;
    let font = ui
        .allocate_ui_with_layout(
            Vec2::new(FAMILY_WIDTH, CONTROL_HEIGHT),
            egui::Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_max_width(FAMILY_WIDTH);
                egui::ComboBox::from_id_salt("classic-font-family")
                    .width(FAMILY_WIDTH)
                    .height(popup_height)
                    .truncate()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .selected_text(&*family)
                    .show_ui(ui, |ui| {
                        ui.set_height(popup_height);
                        let find = ui.add_sized(
                            [ui.available_width(), CONTROL_HEIGHT],
                            egui::TextEdit::singleline(search).hint_text("Search fonts"),
                        );
                        find.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::TextInput,
                                ui.is_enabled(),
                                "Search fonts",
                            )
                        });
                        ui.separator();
                        let filter = search.to_lowercase();
                        let filtered: Vec<_> = families
                            .iter()
                            .filter(|name| name.to_lowercase().contains(&filter))
                            .collect();
                        let mut list = egui::ScrollArea::vertical()
                            .id_salt("font-family-list")
                            .max_height((popup_height - 76.0).max(40.0))
                            .auto_shrink([false, false]);
                        if find.changed() {
                            list = list.vertical_scroll_offset(0.0);
                        }
                        if filtered.is_empty() {
                            list.show(ui, |ui| {
                                if !loading {
                                    ui.weak("No matching fonts");
                                }
                            });
                        } else {
                            let row_height = CONTROL_HEIGHT;
                            list.show_rows(ui, row_height, filtered.len(), |ui, rows| {
                                for index in rows {
                                    let name = filtered[index];
                                    let response = ui.add_sized(
                                        [ui.available_width(), row_height],
                                        Button::selectable(*family == *name, name).truncate(),
                                    );
                                    selected_outline(ui, &response, *family == *name);
                                    if response.clicked() {
                                        family.clone_from(name);
                                        find.surrender_focus();
                                        ui.close();
                                    }
                                }
                            });
                        }
                        let footer = if loading {
                            "Loading installed fonts…".to_owned()
                        } else {
                            format!(
                                "{} {}",
                                filtered.len(),
                                if filtered.len() == 1 { "font" } else { "fonts" }
                            )
                        };
                        ui.add_sized(
                            [ui.available_width(), 20.0],
                            egui::Label::new(RichText::new(footer).weak()).truncate(),
                        );
                        search_response = Some(find);
                    })
            },
        )
        .inner;
    if font.response.clicked()
        && ui.ctx().current_pass_index() == 0
        && let Some(find) = search_response
    {
        find.request_focus();
    }
    font.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), "Font family")
    });
    font.response.on_hover_text("Font family");
}
