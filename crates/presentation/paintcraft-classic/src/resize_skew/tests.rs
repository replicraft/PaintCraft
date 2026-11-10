use super::*;
use egui_kittest::{Harness, kittest::Queryable};
struct State {
    draft: Draft,
    response: Response,
    skin: crate::ClassicVariant,
    fonts_installed: bool,
}
fn harness(skin: crate::ClassicVariant, draft: Draft) -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(800.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                if !state.fonts_installed {
                    paintcraft_ui::fonts::install(ui.ctx());
                    state.fonts_installed = true;
                }
                if state.response != Response::Pending {
                    return;
                }
                let response = dialog_for_skin(ui.ctx(), state.skin, &mut state.draft, 1048576);
                if ui.ctx().current_pass_index() == 0 {
                    state.response = response;
                }
            },
            State {
                draft,
                response: Response::Pending,
                skin,
                fonts_installed: false,
            },
        )
}
#[test]
fn both_real_dialog_layouts_emit_apply_and_cancel_without_changing_parameters() {
    for skin in crate::ClassicVariant::ALL {
        let mut draft = Draft::new((2, 2));
        draft.edit_size(0, 200.0, 1048576);
        draft.skew = [45, 0];
        let expected = Parameters {
            width: 4,
            height: 4,
            shear: (65536, 0),
        };
        assert_eq!(draft.parameters(1048576), Some(expected));
        let mut h = harness(skin, draft);
        h.run();
        h.get_by_label("OK").click();
        h.run();
        assert_eq!(h.state().response, Response::Apply(expected));
        let mut h = harness(skin, Draft::new((2, 2)));
        h.run();
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(h.state().response, Response::Cancel);
    }
}
#[test]
fn units_preserve_the_same_transform_and_unrepresentable_bounding_boxes_disable_apply() {
    let mut d = Draft::new((3, 7));
    d.edit_size(0, 200.0, 1048576);
    let before = d.parameters(1048576);
    d.pixels = true;
    assert_eq!(d.parameters(1048576), before);
    d.linked = false;
    d.edit_size(0, 3.0, 1048576);
    assert_eq!(d.parameters(1048576).unwrap().width, 3);
    d.skew = [89, 89];
    assert!(d.parameters(32).is_none());
    d.scale[0] = f64::NAN;
    assert!(d.parameters(1048576).is_none());
}
