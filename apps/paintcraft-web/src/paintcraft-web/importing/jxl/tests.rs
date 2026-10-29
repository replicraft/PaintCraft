use super::*;
#[test]
fn actual_integer_decoder_results_require_matching_rust_report_and_source_binding() {
    const EIGHT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl"
    ));
    const SIXTEEN: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba16.jxl"
    ));
    let owner = wire::Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 9,
        frontend: 3,
    };
    for (bytes, depth, expected) in [
        (
            EIGHT,
            8,
            vec![
                10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150,
                254, 160, 170, 180, 255,
            ],
        ),
        (
            SIXTEEN,
            16,
            vec![
                0, 0, 255, 1, 1, 127, 128, 0, 255, 4, 8, 128, 16, 32, 64, 255,
            ],
        ),
    ] {
        let request = wire::Request::capture_for(
            wire::Codec::JxlIntegerRgba8,
            Arc::new(bytes.to_vec()),
            owner,
            wire::PIXELS_MAX,
        )
        .unwrap();
        let output = wire::jxl_integer::execute(request.header, bytes, &|| 0).unwrap();
        let source = Source {
            name: "fixture.jxl".into(),
            length: bytes.len() as u64,
        };
        let (_, frame) = response::validate(
            &source,
            owner,
            request.header.nonce,
            &request.header.encode(),
            &output,
            &request.header.sha256,
        )
        .unwrap();
        assert_eq!(frame.jxl_report.unwrap().source_depth, depth);
        assert_eq!(frame.pixels, wire::Pixels::SrgbRgba8(&expected));
        let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
        let document =
            response::assemble(request.header, frame, budget.clone(), &|| Ok(())).unwrap();
        assert!(budget.usage().reserved > 0);
        drop(document);
        assert_eq!(budget.usage().reserved, 0);
        let report_at = wire::RESPONSE_HEADER + env!("CARGO_PKG_VERSION").len();
        for at in [
            report_at + 10,
            report_at + 20,
            report_at + 52,
            report_at + 84,
            output.len() - 1,
        ] {
            let mut changed = output.clone();
            changed[at] ^= 0xff;
            assert!(
                response::validate(
                    &source,
                    owner,
                    request.header.nonce,
                    &request.header.encode(),
                    &changed,
                    &request.header.sha256
                )
                .is_err()
            );
        }
    }
}
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
        report: wire::jxl_integer::Report {
            source_depth: 8,
            width: 1,
            height: 1,
            source_sha256: [0; 32],
            output_sha256: Sha256::digest([10, 20, 30, 0]).into(),
            changed_rgb_components: 0,
            changed_alpha: 0,
        },
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
            report: wire::jxl_integer::Report {
                source_depth: 8,
                width: 1,
                height: 1,
                source_sha256: [0; 32],
                output_sha256: Sha256::digest([1, 2, 3, 4]).into(),
                changed_rgb_components: 0,
                changed_alpha: 0,
            },
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
    let depth = std::env::var("PAINTCRAFT_JXL_PCND_DEPTH").unwrap_or_else(|_| "8".into());
    let (canvas, expected) = match depth.as_str() {
        "8" => (
            (3, 2),
            vec![
                [10, 20, 30, 0],
                [40, 50, 60, 1],
                [70, 80, 90, 127],
                [100, 110, 120, 128],
                [130, 140, 150, 254],
                [160, 170, 180, 255],
            ],
        ),
        "16" => (
            (2, 2),
            vec![
                [0, 0, 255, 1],
                [1, 127, 128, 0],
                [255, 4, 8, 128],
                [16, 32, 64, 255],
            ],
        ),
        _ => panic!("Select exact independent fixture depth8 or16"),
    };
    assert_eq!(document.canvas, canvas);
    assert_eq!(document.roots.len(), 1);
    let raster = &document.layers_in_order().next().unwrap().raster;
    for (index, pixel) in expected.into_iter().enumerate() {
        assert_eq!(
            raster
                .get(
                    (index % canvas.0 as usize) as i64,
                    (index / canvas.0 as usize) as i64
                )
                .0,
            pixel
        );
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

#[test]
fn shared_quarantine_banner_names_image_processing_for_every_language_and_codec() {
    use egui_kittest::{Harness, kittest::Queryable};
    use paintcraft_ui::i18n::{Language, set_language, translate};
    for language in Language::ALL {
        for tiff in [false, true] {
            let mut app = WebApp::new(&eframe::egui::Context::default());
            let source = app.editor.session.snapshot();
            app.jxl.quarantined = true;
            app.tiff.quarantined = tiff;
            let mut h = Harness::builder()
                .with_size(eframe::egui::vec2(1400.0, 300.0))
                .build_ui_state(
                    move |ui, app: &mut WebApp| {
                        set_language(ui.ctx(), language);
                        app.jxl_panel(ui);
                    },
                    app,
                );
            h.run();
            h.get_by_label(&translate(language, CODEC_PAUSED));
            h.get_by_label(&translate(language, "Download project"));
            assert!(Arc::ptr_eq(&source, &h.state().editor.session.snapshot()));
        }
    }
}
