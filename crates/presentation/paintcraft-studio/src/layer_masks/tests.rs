use super::*;
use paintcraft_ui::i18n::{Language, translate};
#[test]
fn active_empty_is_captureable_and_zero_disabled_masks_still_have_all_slot_actions() {
    for selection in [
        SelectionState::Inactive,
        SelectionState::Empty,
        SelectionState::Active,
    ] {
        for mask in [
            None,
            Some(Summary {
                enabled: false,
                domain: Rect::new(-1, -2, 4, 3).unwrap(),
                bands: 0,
                runs: 0,
            }),
        ] {
            let context = Context {
                available: true,
                selection,
                mask,
            };
            assert_eq!(
                context.enabled(Action::UseSelection),
                selection != SelectionState::Inactive
            );
            for action in [Action::SetEnabled(true), Action::Invert, Action::Remove] {
                assert_eq!(context.enabled(action), mask.is_some());
                assert!(
                    !Context {
                        available: false,
                        ..context
                    }
                    .enabled(action)
                );
            }
            assert!(
                !Context {
                    available: false,
                    ..context
                }
                .enabled(Action::UseSelection)
            );
        }
    }
}
#[test]
fn all_saved_mask_captions_have_seven_real_translations() {
    for language in Language::ALL {
        for source in [
            "Layer mask",
            "Use selection as layer mask",
            "Enable layer mask",
            "Invert layer mask",
            "Remove layer mask",
            "No saved layer mask",
            "Saved layer mask domain",
            "Saved layer mask bands",
            "Saved layer mask runs",
            "Pixels outside the saved mask stay hidden. Changing the selection does not change this mask.",
        ] {
            let text = translate(language, source);
            assert!(!text.trim().is_empty());
            if language != Language::English {
                assert_ne!(text, source, "{language:?}: {source}");
            }
        }
    }
}
