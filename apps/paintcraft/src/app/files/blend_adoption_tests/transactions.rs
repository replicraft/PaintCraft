use super::*;

#[test]
fn native_image_picker_and_raw_layer_picker_keep_source_and_history() {
    use paintcraft_core::types::picker::SampleSource;
    for asynchronous in [false, true] {
        for mode in [Mode::Multiply, Mode::Screen] {
            let decorated = asynchronous;
            let doc = source(mode, 2, 1, decorated);
            let mut app = App::new(None);
            app.install(Editor::with_document(doc.clone(), None))
                .unwrap();
            app.editor.set_target(*doc.roots.last().unwrap());
            if asynchronous {
                app.initialize_editing(app.context.clone());
            }
            let before = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            app.editor.options.picker_source = SampleSource::Image;
            app.editor.select_tool(Tool::ColorPicker);
            app.editor.pointer_down((0, 0));
            app.editor.pointer_up();
            app.editor.run_pending();
            wait_edits(&mut app);
            assert_eq!(app.editor.color, Rgba(expected(mode, decorated)));
            app.editor.options.picker_source = SampleSource::Layer;
            app.editor.select_tool(Tool::ColorPicker);
            app.editor.pointer_down((0, 0));
            app.editor.pointer_up();
            app.editor.run_pending();
            wait_edits(&mut app);
            assert_eq!(app.editor.color, Rgba([204, 77, 13, 255]));
            assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
            assert!(app.editor.document().same_semantics(&doc));
            assert_eq!(app.editor.history_view().ticket, ticket);
            assert_eq!(app.editor.undo_depth(), 0);
        }
    }
}

#[test]
fn actual_flatten_saves_minimum_and_undo_redo_restore_pcnd8_intent() {
    let folder = Folder::new();
    for asynchronous in [false, true] {
        for mode in [Mode::Multiply, Mode::Screen] {
            let decorated = asynchronous;
            let doc = source(mode, 2, 1, decorated);
            let mut app = App::new(None);
            app.install(Editor::with_document(doc.clone(), None))
                .unwrap();
            if asynchronous {
                app.initialize_editing(app.context.clone());
            }
            let before = app.editor.document_snapshot();
            app.editor.request(0x6291, Action::FlattenImage);
            app.editor.run_pending();
            wait_edits(&mut app);
            let flat = app.editor.document_snapshot();
            assert_eq!(flat.roots.len(), 1);
            let layer = &flat.layers[&flat.roots[0]];
            assert_eq!(layer.blend_mode, Mode::Normal);
            assert!(layer.filters.is_empty());
            assert!(layer.mask.is_none());
            assert_eq!(
                paintcraft_render::render_flat(&flat).unwrap(),
                expected(mode, decorated).repeat(2)
            );
            assert_eq!(app.editor.undo_depth(), 1);
            assert!(before.same_semantics(&doc));
            let path = folder.0.join(format!("flat-{asynchronous}-{mode:?}.pcnd"));
            save_native(&mut app, path.clone());
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(
                paintcraft_formats::pcnd::probe(&bytes)
                    .unwrap()
                    .container_version,
                1
            );
            assert!(
                Editor::open(&path)
                    .unwrap()
                    .document()
                    .same_semantics(&flat)
            );
            app.editor.request(0x6292, Action::Undo);
            app.editor.run_pending();
            wait_edits(&mut app);
            assert!(app.editor.document().same_semantics(&doc));
            assert_eq!(app.editor.undo_depth(), 0);
            assert_eq!(app.editor.redo_depth(), 1);
            app.editor.request(0x6293, Action::Redo);
            app.editor.run_pending();
            wait_edits(&mut app);
            assert!(app.editor.document().same_semantics(&flat));
            assert_eq!(app.editor.undo_depth(), 1);
            assert_eq!(app.editor.redo_depth(), 0);
        }
    }
}
