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

fn at(h: &Harness<App>, x: f32, y: f32) -> Pos2 {
    h.state().canvas_rect().expect("canvas drawn").min + Vec2::new(x + 0.5, y + 0.5)
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
#[path = "gui/colors.rs"]
mod colors;
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

#[path = "gui/classic_zoom.rs"]
mod classic_zoom;

#[path = "gui/canvas_settings.rs"]
mod canvas_settings;
