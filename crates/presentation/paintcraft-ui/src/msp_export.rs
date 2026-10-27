//! Shared MSP version choice; hosts own admission, preparation and publication.
//! This module deliberately has no format-service or file-system dependency.
use crate::t;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Version {
    #[default]
    UncompressedV1,
    RowRleV2,
}

fn label(version: Version) -> &'static str {
    match version {
        Version::UncompressedV1 => "Version 1 (uncompressed)",
        Version::RowRleV2 => "Version 2 (compressed)",
    }
}

/// Returns true when the choice changes. Hosts must retire any previous
/// prepared artifact, preview, consent and destination before preparing again.
pub fn controls(ui: &mut egui::Ui, version: &mut Version) -> bool {
    let before = *version;
    let ctx = ui.ctx().clone();
    egui::ComboBox::from_label(t!(&ctx, "MSP version"))
        .selected_text(t!(&ctx, label(*version)))
        .show_ui(ui, |ui| {
            for choice in [Version::UncompressedV1, Version::RowRleV2] {
                ui.selectable_value(version, choice, t!(&ctx, label(choice)));
            }
        });
    *version != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Language, set_language, translate};

    fn collect(shape: &egui::epaint::Shape, labels: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => labels.push(text.galley.text().to_owned()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, labels);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn both_versions_render_without_changing_the_selection_in_english_and_japanese() {
        assert_eq!(Version::default(), Version::UncompressedV1);
        for language in [Language::English, Language::Japanese] {
            for mut version in [Version::UncompressedV1, Version::RowRleV2] {
                let ctx = egui::Context::default();
                set_language(&ctx, language);
                let expected = version;
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    assert!(!controls(ui, &mut version));
                });
                output.textures_delta.clear();
                assert_eq!(version, expected);
                let mut labels = Vec::new();
                for shape in output.shapes {
                    collect(&shape.shape, &mut labels);
                }
                for source in ["MSP version", label(expected)] {
                    assert!(labels.contains(&translate(language, source)));
                }
            }
        }
    }

    #[test]
    fn version_choices_are_translated_in_all_seven_languages() {
        for language in Language::ALL {
            if language == Language::English {
                continue;
            }
            for source in [
                "MSP version",
                label(Version::UncompressedV1),
                label(Version::RowRleV2),
            ] {
                assert_ne!(
                    translate(language, source),
                    source,
                    "{}: {source}",
                    language.id()
                );
            }
        }
    }
}
