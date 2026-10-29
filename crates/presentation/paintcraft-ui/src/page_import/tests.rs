use super::*;
#[test]
fn profiled_modal_never_selects_a_page_implicitly() {
    for pages in [
        vec![PageChoice {
            page: Page {
                index: 0,
                width: 1,
                height: 1,
            },
            supported: true,
        }],
        vec![
            PageChoice {
                page: Page {
                    index: 0,
                    width: 1,
                    height: 1,
                },
                supported: false,
            },
            PageChoice {
                page: Page {
                    index: 1,
                    width: 2,
                    height: 1,
                },
                supported: true,
            },
        ],
    ] {
        let ctx = egui::Context::default();
        let mut selected = None;
        for _ in 0..2 {
            let mut response = Response::Pending;
            let mut output = ctx.run_ui(Default::default(), |ui| {
                response = dialog_profiled(
                    ui.ctx(),
                    Format::Tiff,
                    "pages.tiff",
                    &pages,
                    &mut selected,
                    100,
                    None,
                );
            });
            output.textures_delta.clear();
            assert_eq!(response, Response::Pending);
            assert_eq!(selected, None);
        }
    }
}
#[test]
fn qualified_later_page_and_legacy_ids_remain_independent() {
    assert_eq!(Format::Dcx.modal_id(), "dcx-page-import");
    assert_eq!(Format::Dcx.list_id(), "dcx-page-list");
    assert_ne!(Format::Dcx.modal_id(), Format::Tiff.modal_id());
    let choices = [
        PageChoice {
            page: Page {
                index: 0,
                width: 1,
                height: 1,
            },
            supported: false,
        },
        PageChoice {
            page: Page {
                index: 1,
                width: 2,
                height: 1,
            },
            supported: true,
        },
    ];
    let ctx = egui::Context::default();
    let mut selected = Some(1);
    let mut output = ctx.run_ui(Default::default(), |ui| {
        assert_eq!(
            dialog_profiled(
                ui.ctx(),
                Format::Tiff,
                "pages.tiff",
                &choices,
                &mut selected,
                100,
                None
            ),
            Response::Pending
        );
    });
    output.textures_delta.clear();
    assert_eq!(selected, Some(1));
}
#[test]
fn page_profile_labels_and_loss_are_translated_in_all_locales() {
    use crate::i18n::{Language, format_message, translate};
    for language in Language::ALL {
        for source in [
            "This page uses an unsupported TIFF profile.",
            "This page cannot be imported.",
            "Only the selected TIFF page will be imported.",
        ] {
            if language != Language::English {
                assert_ne!(translate(language, source), source);
            }
        }
        let message = format_message(language, "TIFF pages: { $count }", &[("count", "2".into())]);
        assert!(message.contains('2'));
        assert!(!message.contains('$'));
    }
}
