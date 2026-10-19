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
