//! Headless GUI tests: real egui input events through the real layout code.
//! They prove input routing, exactly-once execution and actual-pixel mapping.
//! They do not prove pen/IME behavior, GPU presentation or platform windowing;
//! those need native runs (see docs/IMPLEMENTATION_NOTES.md).

use std::path::PathBuf;

use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use paintcraft::App;
use paintcraft_core::types::Rgba;

fn harness_with(app: App, ppp: f32, two_passes: bool) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .with_pixels_per_point(ppp)
        .build_ui_state(
            move |ui, app: &mut App| {
                app.ui(ui);
                if two_passes && ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("test: force a second layout pass");
                }
            },
            app,
        );
    h.run();
    h
}

fn harness() -> Harness<'static, App> {
    harness_with(App::new(None), 1.0, false)
}

/// Classic v11 keeps New/Open and picture export in its File menu.
fn classic_file_action(h: &mut Harness<App>, label: &str) {
    let role = eframe::egui::Role::Button;
    h.get_by_role_and_label(role, "File").click();
    h.run();
    if label == "Picture…" {
        h.get_by_role_and_label(role, "File")
            .get_by_label("Save as")
            .hover();
        h.run();
    }
    h.get_by_role_and_label(role, "File")
        .get_by_label_contains(label)
        .click();
    h.run();
}

fn at(h: &Harness<App>, x: f32, y: f32) -> Pos2 {
    h.state().canvas_rect().expect("canvas drawn").min + Vec2::new(x + 0.5, y + 0.5)
}

fn displayed_pixel(h: &Harness<App>, x: i64, y: i64) -> [u8; 4] {
    let region = h.state().rendered_region().unwrap();
    assert!(region.contains(x, y));
    let step = i64::from(h.state().rendered_sample_step().unwrap());
    let width = h.state().rendered_size().unwrap()[0] as i64;
    let index = (((y - region.y0) / step) * width + (x - region.x0) / step) as usize * 4;
    h.state().rendered()[index..index + 4].try_into().unwrap()
}

fn press(pos: Pos2) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }
}

fn release(pos: Pos2) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }
}

fn publications(h: &Harness<App>) -> u64 {
    h.state().editor().publications()
}

fn top_pixel(h: &Harness<App>, x: i64, y: i64) -> Rgba {
    let e = h.state().editor();
    e.document().layers[&e.target().unwrap()].raster.get(x, y)
}

/// Click "Add layer" so drawing happens on a fresh transparent layer.
fn with_layer(h: &mut Harness<App>) {
    if h.query_by_label("Add layer").is_none() {
        h.get_by_role_and_label(eframe::egui::Role::Button, "Layers")
            .click();
        h.run();
    }
    h.get_by_label("Add layer").click();
    h.run();
}

/// A key release, so the next press of the same key is a new press and not a repeat.
fn key_up(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers,
    }
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("paintcraft-gui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn drag(h: &mut Harness<App>, a: (f32, f32), b: (f32, f32)) {
    let (pa, pb) = (at(h, a.0, a.1), at(h, b.0, b.1));
    h.input_mut().events.extend([
        Event::PointerMoved(pa),
        press(pa),
        Event::PointerMoved(pb),
        release(pb),
    ]);
    h.step();
    h.run();
}

#[path = "gui/chrome.rs"]
mod chrome;
#[path = "gui/files_and_recovery.rs"]
mod files_and_recovery;
#[path = "gui/input.rs"]
mod input;
#[path = "gui/studio.rs"]
mod studio;

#[path = "gui/tabs.rs"]
mod tabs;

#[path = "gui/centering.rs"]
mod centering;
#[path = "gui/classic_layers.rs"]
mod classic_layers;
#[path = "gui/classic_shapes.rs"]
mod classic_shapes;
#[path = "gui/classic_status.rs"]
mod classic_status;
#[path = "gui/colors.rs"]
mod colors;
#[path = "gui/confirmations.rs"]
mod confirmations;
#[path = "gui/layers.rs"]
mod layers;
#[path = "gui/properties.rs"]
mod properties;
#[path = "gui/secondary.rs"]
mod secondary;
#[path = "gui/shortcuts.rs"]
mod shortcuts;
#[path = "gui/view.rs"]
mod view;

#[path = "gui/classic_brush.rs"]
mod classic_brush;
#[path = "gui/eraser.rs"]
mod eraser;

#[path = "gui/classic_zoom.rs"]
mod classic_zoom;

#[path = "gui/canvas_settings.rs"]
mod canvas_settings;

#[path = "gui/classic_colors.rs"]
mod classic_colors;

#[path = "gui/appearance.rs"]
mod appearance;

fn switch_interface(h: &mut Harness<App>, label: &str) {
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label(label).click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
}

#[path = "gui/interactions.rs"]
mod interactions;

#[path = "gui/font_popup.rs"]
mod font_popup;
#[path = "gui/font_text.rs"]
mod font_text;

#[path = "gui/ui_hover.rs"]
mod ui_hover;

#[path = "gui/tool_interactions.rs"]
mod tool_interactions;

#[path = "gui/classic_dropdowns.rs"]
mod classic_dropdowns;

/// Classic v11 uses the ribbon dropdown; older variants use the inline style.
fn filled_shape(h: &mut Harness<App>) {
    if let Some(button) = h.query_by_label("Filled") {
        button.click();
    } else {
        h.get_by_label("Shape outline").click();
        h.run();
        h.get_by_label("No outline").click();
    }
    h.run();
}

#[path = "gui/workspace_view.rs"]
mod workspace_view;

#[path = "gui/rulers.rs"]
mod rulers;

#[path = "gui/clear_canvas.rs"]
mod clear_canvas;

#[path = "gui/classic_menus.rs"]
mod classic_menus;

#[path = "gui/toolbar_visibility.rs"]
mod toolbar_visibility;

#[path = "gui/dropdown_states.rs"]
mod dropdown_states;

#[path = "gui/selection_menu.rs"]
mod selection_menu;

#[path = "gui/skin_picker.rs"]
mod skin_picker;

#[path = "gui/canvas_handles.rs"]
mod canvas_handles;

#[path = "gui/compact_chrome.rs"]
mod compact_chrome;

#[path = "gui/status_tooltips.rs"]
mod status_tooltips;

#[path = "gui/color_model.rs"]
mod color_model;

#[path = "gui/menu_rows.rs"]
mod menu_rows;

#[path = "gui/tool_headers.rs"]
mod tool_headers;

#[path = "gui/rename_cancel.rs"]
mod rename_cancel;

#[path = "gui/opacity_cancel.rs"]
mod opacity_cancel;

#[path = "gui/layer_previews.rs"]
mod layer_previews;
