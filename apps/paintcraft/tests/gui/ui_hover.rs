use super::*;

#[test]
fn tool_hover_shows_one_tooltip() {
    let mut h = harness();
    h.ctx.style_mut_of(h.ctx.theme(), |s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.get_by_role_and_label(eframe::egui::Role::Button, "Pencil")
        .hover();
    h.run();
    assert_eq!(
        h.query_all_by_role_and_label(eframe::egui::Role::Label, "Pencil")
            .count(),
        1
    );
}

#[test]
fn classic_ribbon_dropdowns_are_compact_and_show_one_tooltip_at_each_display_scale() {
    use eframe::egui::Role;
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let before = h.state().editor().document_snapshot();
        for label in ["Selection options", "Brushes"] {
            let button = h.get_by_role_and_label(Role::Button, label);
            assert!(
                button.rect().height() <= 24.0,
                "{label}: {:?}",
                button.rect()
            );
            // Brushes also has a persistent group caption; count only the
            // additional label introduced by hovering this control.
            let baseline = h.query_all_by_role_and_label(Role::Label, label).count();
            button.hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, label).count(),
                baseline + 1
            );
        }
        h.get_by_role_and_label(Role::Button, "Rectangle").click();
        h.run();
        for label in ["Shape outline", "Shape fill"] {
            let button = h.get_by_role_and_label(Role::Button, label);
            assert!(
                button.rect().width() <= 64.0,
                "{label}: {:?}",
                button.rect()
            );
            assert!(!button.accesskit_node().is_disabled());
            button.hover();
            h.run();
            assert_eq!(h.query_all_by_role_and_label(Role::Label, label).count(), 1);
        }
        assert!(h.state().editor().document().same_semantics(&before));
    }
}
