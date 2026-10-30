//! Reuse the actual raster-review/save controls with independently authored BMP16.
use super::*;

fn cases() -> Vec<Case> {
    let root = fixtures().with_file_name("bmp16-fixtures");
    let oracle = json::parse(&fs::read(root.join("oracle.json")).unwrap()).unwrap();
    oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let name = row.get("file").unwrap().as_str().unwrap().to_owned();
            let bytes = fs::read(root.join(&name)).unwrap();
            let pixels = fs::read(root.join(name.replace(".bmp", ".rgba"))).unwrap();
            assert_eq!(
                hex(&Sha256::digest(&bytes)),
                row.get("sha256").unwrap().as_str().unwrap()
            );
            assert_eq!(
                hex(&Sha256::digest(&pixels)),
                row.get("expected_sha256").unwrap().as_str().unwrap()
            );
            let expected = retained_expected(&bytes, (65, 3), &pixels);
            Case {
                name,
                bytes,
                pixels,
                size: (65, 3),
                expected,
            }
        })
        .collect()
}

#[test]
fn bmp16_native_review_and_loss_free_open_save_reopen_exact_rounded_pixels() {
    let folder = Folder::new();
    for case in cases() {
        let source = folder.0.join(&case.name);
        fs::write(&source, &case.bytes).unwrap();
        let (mut app, ctx) = if case.losses().is_empty() {
            let mut app = App::new(None);
            app.settings.experience = Experience::Studio;
            app.open_path(&source).unwrap();
            assert!(app.pending_import.is_none());
            case.assert_doc(app.editor.document());
            (app, egui::Context::default())
        } else {
            adopt(
                &case,
                &source,
                Experience::Studio,
                ClassicVariant::V11,
                Language::English,
            )
        };
        let copy = folder.0.join(format!("{}.pcnd", case.name));
        save(&mut app, &ctx, &copy);
        app.open_path(&copy).unwrap();
        case.assert_doc(app.editor.document());
        assert_eq!(fs::read(&source).unwrap(), case.bytes);
        if let Some(root) = std::env::var_os("PAINTCRAFT_BMP16_CONSUMER_EVIDENCE") {
            let root = PathBuf::from(root).join("native");
            fs::create_dir_all(&root).unwrap();
            fs::copy(&copy, root.join(format!("{}.pcnd", case.name))).unwrap();
        }
    }
}
