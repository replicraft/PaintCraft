use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};

#[test]
fn opaque_only_preparation_refuses_alpha_before_helper_and_releases_the_shared_charge() {
    for alpha in [0, 128] {
        let budget = SharedBudget::new(3 << 30);
        let service =
            controller::background::Service::with_budget(egui::Context::default(), budget.clone())
                .unwrap();
        let source = Arc::new(
            paintcraft_document::new_document(
                2,
                2,
                Some(Rgba([19, 71, 201, alpha])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap(),
        );
        let owner = wire::Owner {
            session: Uuid::new_v4(),
            state: source.state_id,
            epoch: 0,
            frontend: 3,
        };
        let source_before = source.clone();
        let layout = encode::Layout::new(2, 2, encode::ENCODED_BYTES_MAX).unwrap();
        let pending = service
            .submit_jxr_export(worker::quote(layout).unwrap(), move |cancel| {
                worker::prepare(source, owner, cancel, Policy::Refuse)
            })
            .unwrap();
        assert_eq!(budget.usage().reserved, 2 << 30);
        let until = Instant::now() + Duration::from_secs(5);
        let error = loop {
            if let Some(result) = pending.poll() {
                break result.unwrap_err();
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("opaque composite"));
        assert_eq!(
            source_before
                .layers_in_order()
                .next()
                .unwrap()
                .raster
                .get(0, 0)
                .0,
            [19, 71, 201, alpha]
        );
        drop(pending);
        service.reap_abandoned();
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn abandoned_export_keeps_admission_until_worker_retires_and_oversized_quote_never_runs() {
    let budget = SharedBudget::new(3 << 30);
    let service =
        controller::background::Service::with_budget(egui::Context::default(), budget.clone())
            .unwrap();
    assert!(
        service
            .submit_jxr_export(u64::MAX, |_| unreachable!())
            .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
    let (started, wait) = std::sync::mpsc::channel();
    let pending = service
        .submit_jxr_export(64, move |cancel| {
            started.send(()).unwrap();
            loop {
                cancel.check()?;
                std::thread::sleep(Duration::from_millis(1));
            }
        })
        .unwrap();
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(budget.usage().reserved, 2 << 30);
    drop(pending);
    let until = Instant::now() + Duration::from_secs(5);
    while budget.usage().reserved != 0 {
        service.reap_abandoned();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn frozen_sources_cover_inactive_documents_and_aliases_after_rename_with_bounded_metadata() {
    let root = std::env::temp_dir().join(format!("paintcraft-jxr-source-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("input.pcnd");
    fs::write(&path, b"source bytes").unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.bound = Some(path.clone());
    let ctx = app.context.clone();
    app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
        .unwrap();
    let budget = SharedBudget::new(1 << 20);
    let sources = Sources::capture(&app, &budget).unwrap();
    assert!(budget.usage().reserved > 0);
    let moved = root.join("renamed.pcnd");
    fs::rename(&path, &moved).unwrap();
    let alias = root.join("alias.jxr");
    fs::hard_link(&moved, &alias).unwrap();
    assert_eq!(
        sources.check_destination(&alias).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert!(sources.check_destination(&root.join("copy.jxr")).is_ok());
    assert_eq!(fs::read(&alias).unwrap(), b"source bytes");
    drop(sources);
    assert_eq!(budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn late_canceled_picker_is_not_reinterpreted_as_generic_export_and_portable_inventory_stays_unchanged()
 {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    app.jxr_export_picker = true;
    let ctx = app.context.clone();
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::Export,
            path: Some(PathBuf::from("late.png")),
        },
    );
    assert!(!app.jxr_export_picker && app.confirm.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert!(
        paintcraft_ui::actions::ImageExport::ALL
            .iter()
            .all(|format| format.extension() != "jxr")
    );
    for frontend in [
        registry::Frontend::ClassicV7,
        registry::Frontend::ClassicV10,
        registry::Frontend::ClassicV11,
    ] {
        assert!(admit(frontend, Path::new("image.jxr")).is_err());
    }
    for language in Language::ALL {
        for text in [
            "JPEG XR picture",
            "Save a lossless JPEG XR picture. The picture must be entirely opaque.",
            "Preparing JPEG XR picture…",
            "JPEG XR export requires an entirely opaque picture.",
        ] {
            if language != Language::English {
                assert_ne!(translate(language, text), text, "{language:?} {text}");
            }
        }
    }
}
