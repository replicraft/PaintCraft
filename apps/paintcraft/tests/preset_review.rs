//! Optional window-free review of actual committed shape pixels.
use eframe::icon_data::IconDataExt;
use paintcraft::controller::Editor;
use paintcraft_core::types::Rgba;
use paintcraft_tools::{Action, ShapePreset, Tool};

#[test]
#[ignore = "writes a shape review sheet to /tmp; no windows are opened"]
fn render_closed_shape_presets_for_review() {
    let mut editor = Editor::new_document(900, 450).unwrap();
    for (index, shape) in ShapePreset::ALL.into_iter().enumerate() {
        let x = (index % 6) as i64 * 150 + 20;
        let y = (index / 6) as i64 * 150 + 20;
        editor.request(
            index as u64 + 1,
            Action::Shape {
                layer: editor.target().unwrap(),
                tool: Tool::Preset(shape),
                from: (x, y),
                to: (x + 110, y + 110),
                fill: Some(Rgba([70, 160, 230, 160])),
                stroke: Some((Rgba([20, 30, 40, 255]), 3)),
            },
        );
        editor.run_pending();
    }
    assert_eq!(editor.publications(), 18);
    let icon = eframe::egui::IconData {
        rgba: paintcraft_render::render_flat(editor.document()).unwrap(),
        width: 900,
        height: 450,
    };
    std::fs::write(
        "/tmp/paintcraft-shape-presets.png",
        icon.to_png_bytes().unwrap(),
    )
    .unwrap();
}
