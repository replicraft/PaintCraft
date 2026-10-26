use super::*;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/msp-fixtures/raw-9.msp"
);
#[test]
fn native_msp_consent_preserves_prior_document_and_protects_pending_and_installed_renamed_sources()
{
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let root = std::env::temp_dir().join(format!("paintcraft-native-msp-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("legacy.msp");
    std::fs::write(&path, SOURCE).unwrap();
    let before = app.editor.document_snapshot();
    assert!(app.open_path(&path).is_err());
    let acquire = || {
        crate::format_admission::acquire_resource_source(
            &path,
            paintcraft_formats::msp::BYTES_MAX as u64,
        )
        .unwrap()
    };
    app.stage_msp_import_acquired(acquire(), path.clone())
        .unwrap();
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.stage_msp_import_acquired(acquire(), path.clone())
        .unwrap();
    let moved = root.join("moved.msp");
    std::fs::rename(&path, &moved).unwrap();
    let alias = root.join("alias.pcnd");
    std::fs::hard_link(&moved, &alias).unwrap();
    assert!(app.guard_save_destination(&alias).is_err());
    assert!(app.guard_export_destination(&alias).is_err());
    app.resolve_import(&ctx, true);
    assert_eq!(app.editor.document().canvas, (9, 3));
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert!(app.editor.path().is_none());
    assert!(app.guard_save_destination(&alias).is_err());
    assert!(app.guard_export_destination(&alias).is_err());
    let layer = &app.editor.document().layers[&app.editor.document().roots[0]];
    assert_eq!(layer.raster.get(0, 0), Rgba([255; 4]));
    assert_eq!(layer.raster.get(1, 0), Rgba([0, 0, 0, 255]));
    let flat = export::export_flat(app.editor.document(), ImageFormat::Msp, 90, &[]).unwrap();
    assert_eq!(
        image::decode(&flat.bytes).unwrap().rgba,
        image::decode(SOURCE).unwrap().rgba
    );
    assert_eq!(std::fs::read(moved).unwrap(), SOURCE);
    let alias_export = root.join("alias.msp");
    std::fs::hard_link(root.join("moved.msp"), &alias_export).unwrap();
    app.do_export(&alias_export, ImageFormat::Msp, &[]);
    assert_eq!(std::fs::read(&alias_export).unwrap(), SOURCE);
    let before_export = app.editor.document_snapshot();
    let copy = root.join("fresh.msp");
    app.do_export(&copy, ImageFormat::Msp, &[]);
    let encoded = std::fs::read(&copy).unwrap();
    assert_eq!(&encoded[..4], b"DanM");
    assert_eq!(
        image::decode(&encoded).unwrap().rgba,
        image::decode(SOURCE).unwrap().rgba
    );
    assert!(Arc::ptr_eq(&before_export, &app.editor.document_snapshot()));
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
