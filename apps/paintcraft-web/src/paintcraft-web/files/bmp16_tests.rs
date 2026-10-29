//! Host web frontend adoption and PCND serialization, not browser runtime proof.
use super::*;

#[test]
fn bmp16_web_review_and_loss_free_open_save_reopen_exact_rounded_pixels() {
    let root = fixtures().with_file_name("bmp16-fixtures");
    let oracle = json::parse(&std::fs::read(root.join("oracle.json")).unwrap()).unwrap();
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap().to_owned();
        let bytes = std::fs::read(root.join(&name)).unwrap();
        let pixels = std::fs::read(root.join(name.replace(".bmp", ".rgba"))).unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&pixels)),
            row.get("expected_sha256").unwrap().as_str().unwrap()
        );
        let expected = paintcraft_document::import_image(
            65,
            3,
            &pixels,
            &paintcraft_document::hex(&Sha256::digest(&bytes)),
        )
        .unwrap();
        let case = Case {
            name,
            bytes,
            pixels,
            size: (65, 3),
            expected,
        };
        let mut app = if case.losses().is_empty() {
            let ctx = egui::Context::default();
            let mut app = WebApp::new(&ctx);
            app.frontend = paintcraft_ui::actions::Frontend::Studio;
            app.import_owner = Some(Owner::capture(&app));
            app.accept_loaded(
                decode_for_profile(&case.bytes, case.name.clone(), Frontend::Studio).unwrap(),
                &ctx,
            );
            assert!(app.pending_import.is_none());
            case.assert_doc(app.editor.session.document());
            app
        } else {
            adopt(&case, Frontend::Studio, Language::English)
        };
        let saved =
            encode_quality_for_profile(app.editor.session.document(), None, 90, Frontend::Studio)
                .unwrap();
        case.assert_doc(&pcnd::read(&saved).unwrap());
        let ctx = egui::Context::default();
        app.import_owner = Some(Owner::capture(&app));
        app.accept_loaded(
            decode_for_profile(&saved, "reopened.pcnd".into(), Frontend::Studio).unwrap(),
            &ctx,
        );
        assert!(app.pending_import.is_none());
        case.assert_doc(app.editor.session.document());
        if let Some(root) = std::env::var_os("PAINTCRAFT_BMP16_CONSUMER_EVIDENCE") {
            let root = PathBuf::from(root).join("web");
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join(format!("{}.pcnd", case.name)), &saved).unwrap();
        }
    }
}
