use super::*;
use paintcraft_commands::{LayerRef, StyleOp};
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode, StyleDescriptor};
use paintcraft_document::{StyleInstance, StyleStack};
use paintcraft_tools::StyleEdit;
use std::sync::Arc;

#[test]
fn stored_styles_refuse_browser_adoption_even_when_disabled_or_zero_opacity() {
    for enabled in [false, true] {
        for opacity in [0, 128] {
            let mut doc = new_document(2, 1, Some(WHITE), 1, POLICY).unwrap();
            doc.layers.get_mut(&doc.roots[0]).unwrap().styles = StyleStack::new(vec![
                StyleInstance::new(
                    Uuid::from_u128(3),
                    enabled,
                    StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                        [220, 40, 0],
                        opacity,
                        EffectBlendMode::Normal,
                    )),
                )
                .unwrap(),
            ])
            .unwrap();
            assert_eq!(
                Editor::from_document(doc.clone()).err().unwrap().code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                Editor::from_source8_document(doc).err().unwrap().code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
}

#[test]
fn unsupported_style_action_and_preview_preserve_source_history_and_private_tool() {
    let mut editor = Editor::new(2, 1).unwrap();
    let ticket = editor.session.ticket();
    let edit = StyleEdit {
        owner: editor.session.owner_id(),
        state_id: ticket.state_id,
        publication_epoch: ticket.publication_epoch,
        operation: StyleOp::Add {
            layer: LayerRef::Id(editor.target.unwrap()),
            alias: "overlay".into(),
            descriptor: StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [220, 40, 0],
                128,
                EffectBlendMode::Normal,
            )),
            enabled: true,
            to_index: None,
        },
    };
    // A pending real selection gesture must not be canceled as a side effect.
    editor.tools.tool = paintcraft_tools::Tool::Select;
    editor.tools.pointer_down(editor.target.unwrap(), (0, 0));
    let source = editor.session.snapshot();
    let preview = editor.tools.preview();
    let revision = editor.tools.preview_revision();
    let depth = editor.session.undo_depth();
    assert_eq!(
        editor
            .display_preview(Some(Preview::Style(edit.clone())))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        editor.apply(Action::Style(edit)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(Arc::ptr_eq(&source, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), depth);
    assert_eq!(editor.tools.preview_revision(), revision);
    assert_eq!(editor.tools.preview(), preview);
}
