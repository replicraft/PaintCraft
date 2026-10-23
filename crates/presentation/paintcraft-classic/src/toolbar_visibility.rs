//! Presentation-only reveal state for Classic's automatically hidden toolbar.
use egui::{Rect, Ui};

/// Direct ribbon access to the same persisted choices offered in View.
pub(super) fn menu(
    ui: &mut Ui,
    context: paintcraft_ui::actions::Context,
    actions: &mut Vec<super::Action>,
) {
    use paintcraft_ui::actions::{Action, MenuIconStyle, ViewAction};
    ui.spacing_mut().interact_size.y = 24.0;
    ui.spacing_mut().button_padding = egui::Vec2::ZERO;
    let (response, _) = egui::containers::menu::MenuButton::from_button(
        egui::Button::new("")
            .min_size(egui::vec2(28.0, 24.0))
            .frame_when_inactive(false)
            .frame(true),
    )
    .ui(ui, |ui| {
        ui.set_width(280.0);
        ui.spacing_mut().interact_size.y = 40.0;
        ui.spacing_mut().item_spacing.y = 0.0;
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(16.0));
        }
        for mode in [ViewAction::AlwaysShowToolbar, ViewAction::AutoHideToolbar] {
            let action = Action::View(mode);
            if paintcraft_ui::actions::menu_item_with_icon_style(
                ui,
                action,
                context,
                action.descriptor_for(context.frontend).label,
                22.0,
                MenuIconStyle::ClassicOutline,
            ) {
                actions.push(action);
                ui.close();
            }
        }
    });
    let center = response.rect.center();
    ui.painter().add(egui::Shape::line(
        vec![
            center + egui::vec2(-4.0, -2.0),
            center + egui::vec2(0.0, 2.0),
            center + egui::vec2(4.0, -2.0),
        ],
        egui::Stroke::new(1.0, ui.visuals().strong_text_color()),
    ));
    super::dropdowns::open_state(ui, &response, "Toolbar display options");
    response.on_hover_text("Toolbar display options");
}

#[derive(Debug, Default)]
pub struct ToolbarVisibility {
    collapsed: bool,
    bounds: Option<Rect>,
    frame: Option<u64>,
}

impl ToolbarVisibility {
    pub fn visible(&self) -> bool {
        !self.collapsed
    }

    /// The host includes any font/options rows in the same hover region.
    pub fn remember_bounds(&mut self, bounds: Rect) {
        self.bounds = Some(bounds);
    }

    pub(super) fn update(&mut self, ui: &Ui, menu: Rect, auto_hide: bool) -> bool {
        let frame = ui.ctx().cumulative_frame_nr();
        if self.frame == Some(frame) {
            return self.visible();
        }
        self.frame = Some(frame);
        let previous = self.collapsed;
        if !auto_hide {
            self.collapsed = false;
        } else {
            let (pointer, hold) = ui.input(|input| {
                (
                    input.pointer.hover_pos(),
                    input.pointer.any_down() || input.pointer.any_released(),
                )
            });
            // Hold the previous layout through down/up frames, including a
            // coalesced canvas press/release. A gesture never sees two mappings.
            if !hold {
                if pointer.is_some_and(|point| {
                    menu.contains(point) || self.bounds.is_some_and(|bounds| bounds.contains(point))
                }) {
                    self.collapsed = false;
                } else if !egui::Popup::is_any_open(ui.ctx())
                    && !ui.ctx().egui_wants_keyboard_input()
                {
                    self.collapsed = true;
                }
            }
        }
        if self.collapsed != previous {
            ui.ctx().request_repaint();
        }
        self.visible()
    }
}
