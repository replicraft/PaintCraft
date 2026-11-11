use super::*;
#[test]
fn staged_angle_reuses_exact_cardinals_and_invalid_input_clears_cached_pair() {
    let mut d = Draft::new(Rect::new(-5, 9, 2, 3).unwrap()).unwrap();
    assert_eq!(d.output_size(), Some((2, 3)));
    d.set_degrees(90.0);
    assert_eq!(d.coefficients(), Some((0, 1 << 30)));
    assert_eq!(d.output_size(), Some((3, 2)));
    let old = d.clone();
    d.set_degrees(90.0);
    assert_eq!(d, old);
    d.set_degrees(f64::NAN);
    assert_eq!(d.coefficients(), None);
    assert_eq!(d.output_size(), None);
    d.set_degrees(360.0);
    assert_eq!(d.coefficients(), Some((1 << 30, 0)));
    assert_eq!(d.output_size(), Some((2, 3)));
    d.set_degrees(361.0);
    assert_eq!(d.coefficients(), None);
}
#[test]
fn actual_shared_contents_frames_never_emit_apply_without_user_activation() {
    let mut d = Draft::new(Rect::new(0, 0, 2, 3).unwrap()).unwrap();
    d.set_degrees(36.87);
    let expected = d.coefficients();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert_eq!(contents(ui, &mut d), Response::Pending);
        });
        output.textures_delta.clear();
        assert_eq!(d.coefficients(), expected);
    }
}

#[test]
fn all_supported_languages_localize_new_rotation_controls_and_dimensions() {
    for language in crate::i18n::Language::ALL {
        let output = crate::i18n::format_message(
            language,
            "Output: { $width } × { $height } pixels",
            &[("width", "3".into()), ("height", "2".into())],
        );
        assert!(output.contains('3') && output.contains('2'));
        assert!(!output.contains("$width"));
        for label in [
            "Angle",
            "Positive angles rotate clockwise.",
            "Choose an angle whose output fits the canvas limits.",
            "Pixels change only when you apply this rotation.",
        ] {
            let actual = crate::i18n::translate(language, label);
            if language != crate::i18n::Language::English {
                assert_ne!(actual, label);
            }
        }
    }
}
