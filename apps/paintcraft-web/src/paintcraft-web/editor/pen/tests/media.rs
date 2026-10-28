use super::*;
use paintcraft_core::types::pen::PressureCurve;

#[test]
fn opted_in_media_pressure_has_one_actual_editor_transaction_and_exact_preview_undo_redo() {
    for media in BrushMedia::ALL {
        let mut outputs = Vec::new();
        for varying in [false, true] {
            let mut editor = setup();
            editor.tools.options.media = media;
            editor.tools.options.pen.media_dynamics = true;
            editor.tools.options.pen.opacity_pressure = true;
            editor.tools.options.pen.curve = PressureCurve::Firm;
            editor.tools.options.brush_opacity = 60;
            let budget = SharedBudget::new(64 << 20);
            let baseline = paintcraft_render::render_flat(editor.session.document()).unwrap();
            let depth = editor.session.undo_depth();
            for (sequence, phase, changing) in [(1, Phase::Down, 0.2), (2, Phase::Move, 0.8)] {
                assert!(
                    editor
                        .pen_sample(
                            sample(sequence, phase, Some(if varying { changing } else { 1.0 })),
                            &budget
                        )
                        .unwrap()
                );
            }
            let preview =
                paintcraft_render::render_flat(&editor.display_document().unwrap().unwrap())
                    .unwrap();
            assert_ne!(preview, baseline, "{media:?}");
            assert_eq!(
                paintcraft_render::render_flat(editor.session.document()).unwrap(),
                baseline
            );
            assert_eq!(editor.session.undo_depth(), depth);
            editor
                .pen_sample(
                    sample(3, Phase::Up, Some(if varying { 0.8 } else { 1.0 })),
                    &budget,
                )
                .unwrap();
            assert_eq!(
                paintcraft_render::render_flat(editor.session.document()).unwrap(),
                preview,
                "{media:?}"
            );
            assert_eq!(editor.session.undo_depth(), depth + 1);
            assert_eq!(
                editor
                    .session
                    .history_view()
                    .entries
                    .last()
                    .unwrap()
                    .label
                    .text(),
                "Brush"
            );
            editor.apply(Action::Undo).unwrap();
            assert_eq!(
                paintcraft_render::render_flat(editor.session.document()).unwrap(),
                baseline
            );
            editor.apply(Action::Redo).unwrap();
            assert_eq!(
                paintcraft_render::render_flat(editor.session.document()).unwrap(),
                preview
            );
            outputs.push(preview);
        }
        assert_ne!(
            outputs[0], outputs[1],
            "{media:?} pressure must affect actual pixels"
        );
    }
}
