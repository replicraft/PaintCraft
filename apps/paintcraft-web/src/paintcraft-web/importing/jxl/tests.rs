use super::*;
#[test]
fn browser_jxl_has_no_generic_open_or_classic_route() {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
        Frontend::Studio,
    ] {
        assert!(
            !registry::admission(Request::new(
                frontend,
                Operation::Open,
                Family::Jxl,
                Platform::Web
            ))
            .available()
        );
        assert!(
            !registry::admission(Request::new(
                frontend,
                Operation::ImportConvert,
                Family::Jxl,
                Platform::Web
            ))
            .available()
        );
    }
}
#[test]
fn rust_response_binding_preserves_hidden_rgb_and_charges_actual_resident_tiles() {
    use paintcraft_core::types::resources::SharedBudget;
    use sha2::{Digest, Sha256};
    let source = Source {
        name: "painting.jxl".into(),
        length: 3,
    };
    let owner = wire::Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 7,
        frontend: 3,
    };
    let nonce = Uuid::new_v4();
    let digest: [u8; 32] = Sha256::digest([1, 2, 3]).into();
    let header = wire::Header {
        codec: wire::Codec::JxlRgba8,
        nonce,
        owner,
        sha256: digest,
        length: 3,
        pixels: wire::PIXELS_MAX as u32,
    };
    let pixels = [10, 20, 30, 0, 40, 50, 60, 137];
    let bytes = wire::success(
        header,
        wire::Success {
            width: 2,
            height: 1,
            pixels: wire::Pixels::SrgbRgba8(&pixels),
        },
        0,
    )
    .unwrap();
    let budget = SharedBudget::new(1 << 20);
    let doc = response::document(
        &source,
        owner,
        nonce,
        &header.encode(),
        &bytes,
        &digest,
        budget.clone(),
    )
    .unwrap();
    assert_eq!(
        doc.layers_in_order().next().unwrap().raster.get(0, 0).0,
        [10, 20, 30, 0]
    );
    assert_eq!(
        budget.usage().reserved,
        paintcraft_raster::RasterLayer::TILE_BYTES as u64
    );
    let session = paintcraft_editor::session::Session::new(doc);
    assert!(budget.usage().reserved > 0);
    drop(session);
    assert_eq!(budget.usage().reserved, 0);
    for changed in [0, 1, 2] {
        let mut h = header;
        let mut sha = digest;
        if changed == 0 {
            h.nonce = Uuid::new_v4();
        } else if changed == 1 {
            h.owner.epoch += 1;
        } else {
            sha[0] ^= 1;
        }
        assert!(
            response::document(
                &source,
                owner,
                nonce,
                &h.encode(),
                &bytes,
                &sha,
                budget.clone()
            )
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let mut invalid = bytes.clone();
    invalid[12] ^= 1;
    assert!(
        response::document(
            &source,
            owner,
            nonce,
            &header.encode(),
            &invalid,
            &digest,
            budget.clone()
        )
        .is_err()
    );
    let limit = SharedBudget::new(1);
    assert!(
        response::document(
            &source,
            owner,
            nonce,
            &header.encode(),
            &bytes,
            &digest,
            limit.clone()
        )
        .is_err()
    );
    assert_eq!(limit.usage().reserved, 0);
}
#[test]
fn foreign_file_work_does_not_lose_original_import_owner_or_project() {
    let context = eframe::egui::Context::default();
    let mut app = WebApp::new(&context);
    app.import_owner = Some(Owner::capture(&app));
    app.files.busy = true;
    let before = app.editor.session.snapshot();
    assert!(app.take_jxl_owner().is_none());
    assert!(app.blocked());
    assert!(app.import_owner.as_ref().unwrap().current(&app));
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
}
#[test]
fn completed_import_uses_normal_editor_preferences_and_project_copy_name() {
    let context = eframe::egui::Context::default();
    let mut app = WebApp::new(&context);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.controls.options.size = 17;
    let owner = Owner::capture(&app);
    let cancelled = Rc::new(Cell::new(false));
    let document = paintcraft_document::import_image(1, 1, &[10, 20, 30, 0], "fixture").unwrap();
    let result = Rc::new(RefCell::new(Some(Ok(Prepared {
        document,
        name: "painting.jxl".into(),
    }))));
    app.jxl.pending = Some(Pending {
        owner,
        cancelled,
        #[cfg(target_arch = "wasm32")]
        cancellation: Default::default(),
        result,
    });
    app.poll_jxl(&context);
    assert!(app.editor.session.is_dirty());
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert_eq!(app.controls.options.size, 17);
    assert_eq!(app.download_name(None), "painting.pcnd");
    assert_eq!(
        app.editor
            .session
            .document()
            .layers_in_order()
            .next()
            .unwrap()
            .raster
            .get(0, 0)
            .0,
        [10, 20, 30, 0]
    );
    let bytes = paintcraft_formats::pcnd::write_compact(app.editor.session.document()).unwrap();
    let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
    assert_eq!(
        reopened
            .layers_in_order()
            .next()
            .unwrap()
            .raster
            .get(0, 0)
            .0,
        [10, 20, 30, 0]
    );
}
#[test]
fn stale_completed_import_drops_private_document_without_replacing_newer_work() {
    let context = eframe::egui::Context::default();
    let mut app = WebApp::new(&context);
    let owner = Owner::capture(&app);
    let document = paintcraft_document::import_image(1, 1, &[1, 2, 3, 4], "source").unwrap();
    app.jxl.pending = Some(Pending {
        owner,
        cancelled: Rc::new(Cell::new(false)),
        #[cfg(target_arch = "wasm32")]
        cancellation: Default::default(),
        result: Rc::new(RefCell::new(Some(Ok(Prepared {
            document,
            name: "old.jxl".into(),
        })))),
    });
    app.editor = crate::editor::Editor::new(9, 7).unwrap();
    let before = app.editor.session.snapshot();
    app.poll_jxl(&context);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.document().canvas, (9, 7));
    assert!(!app.jxl.busy());
}

#[test]
#[ignore = "explicit production browser download oracle; requires PAINTCRAFT_JXL_PCND"]
fn downloaded_fixture_keeps_independent_samples() {
    use std::io::Read;
    let path =
        std::env::var_os("PAINTCRAFT_JXL_PCND").expect("explicit browser PCND artifact path");
    let file = std::fs::File::open(path).unwrap();
    assert!(file.metadata().unwrap().len() <= 64 << 20);
    let mut bytes = Vec::new();
    file.take((64 << 20) + 1).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= 64 << 20);
    let document = paintcraft_formats::pcnd::read_with_limits(
        &bytes,
        wire::PIXELS_MAX,
        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
    )
    .unwrap();
    assert_eq!(document.canvas, (3, 2));
    assert_eq!(document.roots.len(), 1);
    let raster = &document.layers_in_order().next().unwrap().raster;
    let expected = [
        [10, 20, 30, 0],
        [40, 50, 60, 1],
        [70, 80, 90, 127],
        [100, 110, 120, 128],
        [130, 140, 150, 254],
        [160, 170, 180, 255],
    ];
    for (index, pixel) in expected.into_iter().enumerate() {
        assert_eq!(raster.get((index % 3) as i64, (index / 3) as i64).0, pixel);
    }
}

#[test]
fn prelaunch_cancel_marks_only_the_owned_pending_request_without_changing_the_document() {
    let ctx = eframe::egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let cancelled = Rc::new(Cell::new(false));
    let result = Rc::new(RefCell::new(None));
    app.jxl.pending = Some(Pending {
        owner: Owner::capture(&app),
        cancelled: cancelled.clone(),
        #[cfg(target_arch = "wasm32")]
        cancellation: Default::default(),
        result: result.clone(),
    });
    app.jxl.cancel();
    assert!(cancelled.get());
    assert!(result.borrow().is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(ticket, app.editor.session.ticket());
}
