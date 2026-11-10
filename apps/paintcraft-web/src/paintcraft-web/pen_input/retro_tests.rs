use super::*;
use paintcraft_ui::actions::Frontend;
#[test]
fn retro_pen_identity_never_aliases_classic_or_studio() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    assert_eq!(app.pen_owner().interface, 0);
    app.frontend = Frontend::Classic;
    for (variant, expected) in [
        (paintcraft_classic::ClassicVariant::V7, 1),
        (paintcraft_classic::ClassicVariant::V10, 2),
        (paintcraft_classic::ClassicVariant::V11, 3),
    ] {
        app.variant = variant;
        assert_eq!(app.pen_owner().interface, expected);
    }
    app.frontend = Frontend::Retro;
    assert_eq!(app.pen_owner().interface, 4);
}
