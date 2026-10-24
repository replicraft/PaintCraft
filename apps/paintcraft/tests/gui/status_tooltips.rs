use super::*;
use eframe::egui::Role;
use paintcraft_ui::actions::{Context, Frontend};

#[test]
fn elided_classic_status_fields_show_one_complete_tooltip() {
    for (ppp, theme) in [
        (1.0, eframe::egui::Theme::Light),
        (2.0, eframe::egui::Theme::Dark),
    ] {
        let message =
            "Imported drawing with a long filename and additional format details".repeat(8);
        let recovery = "Recovery checkpoint: saved locally";
        let tooltip = format!("{message}\n{recovery}");
        let rendered_message = message.clone();
        let mut h = Harness::builder()
            .with_size(Vec2::new(360.0, 260.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                move |ui, actions: &mut usize| {
                    let status = paintcraft_classic::status::Status {
                        message: &rendered_message,
                        recovery: Some(recovery),
                        canvas: Some((1048576, 1048575)),
                        pointer: Some((1000000, 999999)),
                        selection: Some((888888, 777777)),
                    };
                    let context = Context {
                        frontend: Frontend::Classic,
                        document_open: true,
                        can_undo: false,
                        can_redo: false,
                        can_save_all: false,
                        selection_active: true,
                        tool: paintcraft_tools::Tool::Pencil,
                        blocked: false,
                        layer_index: Some(0),
                        layer_count: 1,
                        layer_visible: true,
                        zoom: 1.0,
                        pixel_grid: false,
                        can_fit_selection: true,
                        can_crop_selection: true,
                        status_bar: true,
                        rulers: false,
                        fullscreen: false,
                        auto_hide_toolbar: false,
                        thumbnail: false,
                    };
                    *actions += paintcraft_classic::status::contents(
                        ui,
                        status,
                        context,
                        eframe::egui::Id::unique("status-tooltip-test"),
                    )
                    .len();
                },
                0usize,
            );
        paintcraft_ui::fonts::install(&h.ctx);
        paintcraft_ui::theme::apply(&h.ctx, theme);
        h.ctx.set_theme(theme);
        h.ctx.style_mut_of(theme, |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        h.run();
        let zoom = h.get_by_label("Zoom presets").rect();
        for (raw, hint) in [
            ("1000000, 999999 px", "Cursor position: 1000000, 999999 px"),
            ("888888 × 777777 px", "Selection size: 888888 × 777777 px"),
            ("1048576 × 1048575 px", "Canvas size: 1048576 × 1048575 px"),
        ] {
            h.get_by_role_and_label(Role::Label, raw).hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, raw).count(),
                1,
                "raw field text must not appear in an automatic second tooltip"
            );
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, hint).count(),
                2,
                "field accessibility label and exactly one complete tooltip"
            );
            assert_eq!(h.get_by_label("Zoom presets").rect(), zoom);
        }
        h.get_by_role_and_label(Role::Label, &message).hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, &message).count(),
            1
        );
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, &tooltip).count(),
            1
        );
        assert_eq!(h.get_by_label("Zoom presets").rect(), zoom);
        assert_eq!(*h.state(), 0, "hovering must emit no zoom action");
    }
}
