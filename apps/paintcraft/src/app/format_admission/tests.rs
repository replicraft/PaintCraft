use super::*;

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("paintcraft-admission-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn rejected_source_has_no_document_history_binding_or_recent_side_effects() {
    let dir = scratch();
    let mut app = App::new(None);
    let source = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let png = image::encode(ImageFormat::Png, 1, 1, &[1, 2, 3, 255], 90).unwrap();
    for name in ["foreign.svg", "foreign.pdn", "mismatch.jpg", "unknown.xyz"] {
        let path = dir.join(name);
        std::fs::write(&path, &png).unwrap();
        assert!(app.open_path(&path).is_err());
        assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert!(app.bound.is_none());
        assert!(app.recent.entries.is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn launch_settings_are_loaded_before_profile_admission() {
    let dir = scratch();
    Settings {
        experience: Experience::Studio,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let source = dir.join("unsupported.pdn");
    std::fs::write(&source, b"unqualified pdn").unwrap();
    let app = App::new_with_settings(Some(source), dir.clone());
    assert_eq!(app.format_frontend(), Frontend::Studio);
    assert!(app.editor.status().contains("frontend=Studio"));
    assert!(!app.editor.status().contains("unsupported_frontend_format"));
    assert!(app.bound.is_none());
    assert!(app.editor.path().is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opening_a_launch_document_keeps_brush_selected_without_editing_its_pixels() {
    let dir = scratch();
    let png = image::encode(ImageFormat::Png, 1, 1, &[2, 3, 4, 255], 90).unwrap();
    let editor = Editor::new_document(2, 3).unwrap();
    let pcnd = paintcraft_formats::pcnd::write(editor.document()).unwrap();
    for experience in [Experience::Classic, Experience::Studio] {
        Settings {
            experience,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        for (name, bytes, dimensions) in
            [("image.png", &png, (1, 1)), ("native.pcnd", &pcnd, (2, 3))]
        {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            let app = App::new_with_settings(Some(path), dir.clone());
            assert_eq!(app.editor.tool, Tool::Brush);
            assert_eq!(app.editor.document().canvas, dimensions);
            assert_eq!(app.editor.undo_depth(), 0);
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), *bytes);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bounded_reader_checks_metadata_and_a_growing_stream_without_partial_publication() {
    assert_eq!(read_limited(&b"abcd"[..], 4).unwrap(), b"abcd");
    assert_eq!(
        read_limited(&b"abcde"[..], 4).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let dir = scratch();
    let path = dir.join("huge.png");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(SOURCE_BYTES + 1)
        .unwrap();
    let mut app = App::new(None);
    let source = app.editor.document_snapshot();
    assert_eq!(
        app.open_path(&path).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn qualified_raster_and_pcnd_use_the_same_admitted_snapshot() {
    let dir = scratch();
    let picture = dir.join("image.png");
    std::fs::write(
        &picture,
        image::encode(ImageFormat::Png, 1, 1, &[2, 3, 4, 0], 90).unwrap(),
    )
    .unwrap();
    let mut app = App::new(None);
    app.open_path(&picture).unwrap();
    assert_eq!(app.editor.document().canvas, (1, 1));
    assert!(app.bound.is_none());
    let project = dir.join("native.pcnd");
    std::fs::write(
        &project,
        paintcraft_formats::pcnd::write(app.editor.document()).unwrap(),
    )
    .unwrap();
    app.open_path(&project).unwrap();
    assert_eq!(app.bound, Some(project));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn manual_unqualified_exports_and_bad_launch_never_bind_or_replace_sources() {
    let dir = scratch();
    let mut app = App::new(None);
    let source = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    for (name, format) in [
        ("unsupported.gif", ImageFormat::Gif),
        ("unsupported.tiff", ImageFormat::Tiff),
        ("mismatch.heic", ImageFormat::Png),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, b"sentinel").unwrap();
        app.plan_export_quality(path.clone(), 90);
        assert!(app.confirm.is_none());
        app.do_export_quality(&path, format, &KNOWN_LOSSES.map(str::to_owned), 90);
        assert_eq!(std::fs::read(&path).unwrap(), b"sentinel");
        assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view().ticket, ticket);
    }
    let path = dir.join("invalid.svg");
    std::fs::write(&path, b"<svg/>").unwrap();
    let app = App::new(Some(path));
    assert!(app.editor.path().is_none());
    assert!(app.bound.is_none());
    assert_eq!(app.path_text, "untitled.pcnd");
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn ordinary_fifo_is_refused_before_a_blocking_open() {
    let dir = scratch();
    let path = dir.join("pipe.png");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let source = path.clone();
    std::thread::spawn(move || {
        let _ = send.send(read_source(&source));
    });
    let result = receive
        .recv_timeout(Duration::from_secs(2))
        .expect("FIFO admission must not block on open");
    assert_eq!(result.unwrap_err().code, ErrorCode::StorageFailed);
    std::fs::remove_dir_all(dir).unwrap();
}
