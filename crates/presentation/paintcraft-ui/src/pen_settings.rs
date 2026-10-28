//! Shared controls for captured pen policies; no device capabilities are inferred.
use crate::t;
use paintcraft_core::types::pen::{PressureCurve, Settings};

pub fn show(ui: &mut egui::Ui, values: &mut Settings) {
    ui.checkbox(
        &mut values.size_pressure,
        t!(ui.ctx(), "Use pressure to control size"),
    );
    ui.add_enabled_ui(values.size_pressure, |ui| {
        ui.label(t!(ui.ctx(), "Pressure response"));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(28.0, 12.0);
            for curve in PressureCurve::ALL {
                ui.radio_value(&mut values.curve, curve, t!(ui.ctx(), label(curve)));
            }
        });
        ui.weak(t!(
            ui.ctx(),
            "Soft needs less pressure; Firm needs more pressure."
        ));
    });
    ui.weak(t!(
        ui.ctx(),
        "Applies to plain round brush strokes and pen erasers. Other brushes keep their existing behavior."
    ));
    ui.weak(t!(
        ui.ctx(),
        "Pens without reported pressure use the selected size."
    ));
    ui.weak(t!(ui.ctx(), "Changes apply to new strokes."));
}

fn label(curve: PressureCurve) -> &'static str {
    match curve {
        PressureCurve::Linear => "Linear",
        PressureCurve::Soft => "Soft",
        PressureCurve::Firm => "Firm",
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, translate};

    #[test]
    fn pen_settings_are_translated_in_every_language() {
        for language in Language::ALL {
            for source in [
                "Pen and tablet",
                "Use pressure to control size",
                "Pressure response",
                "Linear",
                "Soft",
                "Firm",
                "Soft needs less pressure; Firm needs more pressure.",
                "Applies to plain round brush strokes and pen erasers. Other brushes keep their existing behavior.",
                "Pens without reported pressure use the selected size.",
                "Changes apply to new strokes.",
            ] {
                let translated = translate(language, source);
                assert!(!translated.is_empty());
                if language != Language::English {
                    assert_ne!(translated, source, "{}: {source}", language.id());
                }
            }
        }
    }
}
