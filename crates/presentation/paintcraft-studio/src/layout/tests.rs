use super::*;

fn frame(ctx: &egui::Context, layout: &Layout, workspace: &mut Workspace) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            ..Default::default()
        },
        |_| {
            layout.install(ctx);
            let mut controls = paintcraft_ui::chrome::Controls {
                tool: paintcraft_tools::Tool::Pencil,
                options: Default::default(),
                primary: paintcraft_core::types::Rgba([0, 0, 0, 255]),
                secondary: paintcraft_core::types::Rgba([255, 255, 255, 255]),
            };
            crate::floating_controls(
                ctx,
                &mut controls,
                workspace,
                true,
                egui::Id::unique("test-document"),
            );
            crate::layers_window(ctx, &mut workspace.layers, 150.0, |ui| {
                ui.label("Layer contents");
            });
            crate::history_window(ctx, &mut workspace.history, &[], 0, true, 150.0);
        },
    );
    output.textures_delta.clear();
}

#[test]
fn fresh_context_restores_independent_utility_positions_and_visibility() {
    let mut layout = Layout::default();
    layout.panels[0].position = Some([140, 180]);
    layout.panels[1].visible = false;
    layout.panels[1].position = Some([80, 600]);
    layout.panels[2].position = Some([950, 420]);
    layout.panels[3].position = Some([900, 180]);
    let mut workspace = Workspace::default();
    layout.restore_visibility(&mut workspace);
    let context = egui::Context::default();
    for _ in 0..3 {
        frame(&context, &layout, &mut workspace);
    }
    let captured = layout.capture(&context, &workspace);
    assert!(!captured.panels[1].visible);
    assert_eq!(
        captured.panels[1].position,
        Some([80, 600]),
        "hidden position survives"
    );
    for index in [0, 2, 3] {
        assert_eq!(
            captured.panels[index].position,
            layout.panels[index].position
        );
    }
    let restarted = egui::Context::default();
    let mut restarted_workspace = Workspace::default();
    captured.restore_visibility(&mut restarted_workspace);
    for _ in 0..3 {
        frame(&restarted, &captured, &mut restarted_workspace);
    }
    assert_eq!(captured.capture(&restarted, &restarted_workspace), captured);
}

#[test]
fn off_screen_saved_positions_are_recovered_within_the_current_viewport() {
    let mut layout = Layout::default();
    for panel in &mut layout.panels {
        panel.position = Some([32768, -32768]);
    }
    let mut workspace = Workspace::default();
    let context = egui::Context::default();
    for _ in 0..3 {
        frame(&context, &layout, &mut workspace);
    }
    let captured = layout.capture(&context, &workspace);
    for panel in &captured.panels {
        let [x, y] = panel.position.unwrap();
        assert!((0..1400).contains(&x), "{panel:?}");
        assert!((150..1000).contains(&y), "{panel:?}");
    }
}

#[test]
fn existing_dragged_window_state_takes_precedence_over_saved_defaults() {
    let context = egui::Context::default();
    let layout = Layout::default();
    let mut workspace = Workspace::default();
    frame(&context, &layout, &mut workspace);
    let mut changed = layout.clone();
    changed.panels[0].position = Some([500, 250]);
    // Supplying different startup defaults during the session must not move a window.
    frame(&context, &changed, &mut workspace);
    assert_ne!(
        changed.capture(&context, &workspace).panels[0].position,
        Some([500, 250])
    );
}
