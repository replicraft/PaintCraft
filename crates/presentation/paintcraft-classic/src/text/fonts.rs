//! Search stays fixed while the family list scrolls. Only visible rows are drawn.
use super::*;
use paintcraft_ui::t;
mod navigation;

pub fn family(
    ui: &mut Ui,
    family: &mut String,
    families: &[String],
    search: &mut String,
    loading: bool,
) {
    family_with_metrics(ui, family, families, search, loading, Metrics::MODERN);
}

pub(super) fn family_with_metrics(
    ui: &mut Ui,
    family: &mut String,
    families: &[String],
    search: &mut String,
    loading: bool,
    metrics: Metrics,
) {
    let popup_height = (ui.ctx().content_rect().height() * 0.5 - 24.0).clamp(120.0, 340.0);
    let mut search_response = None;
    let state_id = ui.make_persistent_id("font-family-was-open");
    let was_open = ui.data(|data| data.get_temp::<bool>(state_id).unwrap_or(false));
    let font = ui
        .allocate_ui_with_layout(
            Vec2::new(metrics.family_width, metrics.height),
            egui::Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_max_width(metrics.family_width);
                egui::ComboBox::from_id_salt("classic-font-family")
                    .width(metrics.family_width)
                    .height(popup_height)
                    .truncate()
                    .icon(|ui, rect, visuals, _| {
                        let center = rect.center();
                        ui.painter().add(egui::Shape::line(
                            vec![
                                center + Vec2::new(-4.0, -2.0),
                                center + Vec2::new(0.0, 2.0),
                                center + Vec2::new(4.0, -2.0),
                            ],
                            egui::Stroke::new(1.0, visuals.fg_stroke.color),
                        ));
                    })
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .selected_text(&*family)
                    .show_ui(ui, |ui| {
                        popup::style(ui, metrics);
                        ui.set_height(popup_height);
                        let search_id = ui.make_persistent_id("font-search");
                        let navigation_id = ui.make_persistent_id("font-navigation");
                        let key = navigation::take_key(ui, search_id);
                        let find = ui.add_sized(
                            [ui.available_width(), metrics.height],
                            egui::TextEdit::singleline(search)
                                .id(search_id)
                                .hint_text(t!(ui.ctx(), "Search fonts"))
                                .event_filter(egui::EventFilter {
                                    vertical_arrows: true,
                                    ..Default::default()
                                }),
                        );
                        let accessible_label = t!(ui.ctx(), "Search fonts");
                        find.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::TextInput,
                                ui.is_enabled(),
                                &accessible_label,
                            )
                        });
                        ui.separator();
                        let header_height = ui.cursor().min.y - ui.min_rect().min.y;
                        let list_height =
                            (popup_height - header_height - 20.0 - ui.spacing().item_spacing.y)
                                .max(metrics.height);
                        let filter = search.to_lowercase();
                        let filtered: Vec<_> = families
                            .iter()
                            .filter(|name| name.to_lowercase().contains(&filter))
                            .collect();
                        let mut navigation = ui.data(|data| {
                            data.get_temp::<navigation::Navigation>(navigation_id).unwrap_or_default()
                        });
                        if find.changed() {
                            navigation.reset(&filtered, None);
                        } else if !was_open {
                            navigation.reset(&filtered, Some(family));
                        } else if navigation.index(&filtered).is_none() {
                            navigation.reset(&filtered, None);
                        }
                        if let Some(key) = key {
                            let page = (list_height / metrics.height).floor().max(1.0) as usize;
                            navigation.move_by(&filtered, key, page);
                            if key == egui::Key::Enter
                                && let Some(index) = navigation.index(&filtered)
                            {
                                family.clone_from(filtered[index]);
                                find.surrender_focus();
                                ui.close();
                            }
                        }
                        let candidate = navigation.index(&filtered);
                        let description = navigation.candidate.as_ref().map_or_else(
                            || t!(ui.ctx(), "No matching fonts"),
                            |name| t!(ui.ctx(), "Keyboard choice: { $name }. Up/Down or Page Up/Down to browse; Enter to apply; Escape to cancel.", &[("name", name.clone())]),
                        );
                        ui.ctx().accesskit_node_builder(find.id, |node| {
                            node.set_description(description);
                        });
                        ui.data_mut(|data| data.insert_temp(navigation_id, navigation));
                        let mut list = egui::ScrollArea::vertical()
                            .id_salt("font-family-list")
                            .max_height(list_height)
                            .auto_shrink([false, false]);
                        if find.changed() {
                            list = list.vertical_scroll_offset(0.0);
                        } else if !was_open || key.is_some() {
                            let index = candidate.unwrap_or(0);
                            let step = metrics.height + ui.spacing().item_spacing.y;
                            let offset = index as f32 * step - (list_height - metrics.height) * 0.5;
                            list = list.vertical_scroll_offset(offset.max(0.0));
                        }
                        if filtered.is_empty() {
                            list.show(ui, |ui| {
                                if !loading {
                                    ui.weak(t!(ui.ctx(), "No matching fonts"));
                                }
                            });
                        } else {
                            let row_height = metrics.height;
                            list.show_rows(ui, row_height, filtered.len(), |ui, rows| {
                                for index in rows {
                                    let name = filtered[index];
                                    let response = popup::row(ui, name, *family == *name, metrics);
                                    if Some(index) == candidate && *family != *name {
                                        ui.painter().rect_stroke(
                                            response.rect.shrink(0.5), 4,
                                            ui.visuals().selection.stroke, egui::StrokeKind::Inside,
                                        );
                                    }
                                    if response.clicked() {
                                        family.clone_from(name);
                                        find.surrender_focus();
                                        ui.close();
                                    }
                                }
                            });
                        }
                        let footer = if loading {
                            t!(ui.ctx(), "Loading installed fonts…")
                        } else {
                            t!(ui.ctx(), "{ $count } fonts", &[("count", filtered.len().to_string())])
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
    let open = egui::ComboBox::is_open(ui.ctx(), font.response.id);
    ui.data_mut(|data| data.insert_temp(state_id, open));
    if font.response.clicked()
        && ui.ctx().current_pass_index() == 0
        && let Some(find) = search_response
    {
        find.request_focus();
    }
    let accessible_label = t!(ui.ctx(), "Font family");
    font.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), &accessible_label)
    });
    font.response.on_hover_text(t!(ui.ctx(), "Font family"));
}
