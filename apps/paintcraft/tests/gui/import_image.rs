use super::*;
use eframe::egui::Role;

#[test]
fn studio_explicit_import_menu_creates_an_unbound_tab_and_retains_the_old_document() {
    let (mut h, dir) = super::studio::studio_harness(true);
    let path = dir.join("picture.qoi");
    let mut bytes = b"qoif".to_vec();
    bytes.extend(1u32.to_be_bytes());
    bytes.extend(1u32.to_be_bytes());
    bytes.extend([4, 0, 0xff, 17, 51, 93, 127, 0, 0, 0, 0, 0, 0, 0, 1]);
    std::fs::write(&path, &bytes).unwrap();
    let source = h.state().editor().document_snapshot();
    let count = h.state().document_count();
    h.state_mut().set_path_text(&path.to_string_lossy());
    h.get_by_role_and_label(Role::Button, "File").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "File")
        .get_by_label("Import image…")
        .click();
    h.run();
    assert_eq!(h.state().document_count(), count + 1);
    assert_eq!(h.state().editor().document().canvas, (1, 1));
    assert_ne!(
        h.state().editor().document().document_id,
        source.document_id
    );
    assert!(h.state().editor().path().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn converted_paint_import_preserves_the_old_tab_until_consent_then_keeps_real_layers() {
    let (mut h, dir) = super::studio::studio_harness(true);
    let bytes = include_bytes!(
        "../../../../crates/adapters/paint-lib/tests/fixtures/mspaintlib-sample.paint"
    );
    let path = dir.join("sample.paint");
    std::fs::write(&path, bytes).unwrap();
    let source = h.state().editor().document_snapshot();
    let count = h.state().document_count();
    for accept in [false, true] {
        h.state_mut().set_path_text(&path.to_string_lossy());
        h.get_by_role_and_label(Role::Button, "File").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "File")
            .get_by_label("Import image…")
            .click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&source));
        assert_eq!(h.state().document_count(), count);
        assert!(
            h.query_by_label("Open with these import limitations?")
                .is_some()
        );
        h.get_by_label(if accept { "Open image" } else { "Cancel" })
            .click();
        h.run();
        if !accept {
            assert!(h.state().editor().document().same_semantics(&source));
        }
    }
    assert_eq!(h.state().document_count(), count + 1);
    assert_eq!(h.state().editor().document().roots.len(), 5);
    assert!(h.state().editor().path().is_none());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pdn_open_and_import_require_consent_keep_real_layers_and_save_as_pcnd() {
    use sha2::{Digest, Sha256};
    for explicit in [false, true] {
        let (mut h, dir) = super::studio::studio_harness(true);
        let bytes = include_bytes!(
            "../../../../crates/adapters/pdn-lib/tests/fixtures/paintdotjs-2-layer.pdn"
        );
        let path = dir.join("layers.pdn");
        std::fs::write(&path, bytes).unwrap();
        let source = h.state().editor().document_snapshot();
        let count = h.state().document_count();
        for accept in [false, true] {
            h.state_mut().set_path_text(&path.to_string_lossy());
            if explicit {
                h.get_by_role_and_label(Role::Button, "File").click();
                h.run();
                h.get_by_role_and_label(Role::Button, "File")
                    .get_by_label("Import image…")
                    .click();
            } else {
                h.get_by_role_and_label(Role::Button, "Open").click();
            }
            h.run();
            assert!(h.state().editor().document().same_semantics(&source));
            assert_eq!(h.state().document_count(), count);
            h.get_by_role_and_label(Role::Button, if accept { "Open image" } else { "Cancel" })
                .click();
            h.run();
            if !accept {
                assert!(h.state().editor().document().same_semantics(&source));
                assert_eq!(h.state().document_count(), count);
            }
        }
        assert_eq!(h.state().document_count(), count + 1);
        assert!(h.state().editor().path().is_none());
        assert!(h.state().editor().is_dirty());
        let doc = h.state().editor().document();
        assert_eq!(doc.canvas, (3, 2));
        assert_eq!(doc.roots.len(), 2);
        for (id, name, visible, opacity, expected) in [
            (
                doc.roots[0],
                "Base 日本語",
                true,
                201,
                "d6931877bd07347276a7149f0ff06c97a16de2b64c416385f72e649facad7c56",
            ),
            (
                doc.roots[1],
                "Overlay é",
                false,
                73,
                "4f570b69e8ee688b9e255dab9f8842d18f632314066f17580b70460852dc50c8",
            ),
        ] {
            let layer = &doc.layers[&id];
            assert_eq!(
                (&*layer.name, layer.visible, layer.opacity),
                (name, visible, opacity)
            );
            let mut hash = Sha256::new();
            for y in 0..2 {
                for x in 0..3 {
                    hash.update(layer.raster.get(x, y).0);
                }
            }
            assert_eq!(paintcraft_document::hex(&hash.finalize()), expected);
        }
        let imported = h.state().editor().document_snapshot();
        let output = dir.join("converted.pcnd");
        h.state_mut().set_path_text(&output.to_string_lossy());
        h.get_by_role_and_label(Role::Button, "Save").click();
        h.run();
        let reopened = paintcraft_formats::pcnd::read(&std::fs::read(output).unwrap()).unwrap();
        assert!(reopened.same_semantics(&imported));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
