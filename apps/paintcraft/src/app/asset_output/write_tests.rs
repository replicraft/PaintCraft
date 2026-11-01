//! Real background staging and UI-like final authorization; no native picker.
use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, Descriptor},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::registry::{Family, Operation};
use std::{fs, time::Instant};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("paintcraft-asset-write-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn entries(&self) -> usize {
        fs::read_dir(&self.0).unwrap().count()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(Instant::now() < deadline, "background write did not retire");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn event(job: &WriteJob) -> WriteEvent {
    let mut value = None;
    until(|| {
        value = job.poll();
        value.is_some()
    });
    value.unwrap()
}
fn app() -> App {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["asset write Noise"]),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(doc, None);
    app
}
#[derive(Clone, Copy)]
enum Decision {
    RevokeBake,
    Cancel,
    Approve,
}
#[test]
fn staged_asset_waits_for_valid_frozen_consent_and_cancel_retires_bytes_and_owner() {
    const OLD: &[u8] = b"existing destination must survive until authorization";
    const NEXT: &[u8] = b"exact prepared encoded asset bytes\0\x80\xff";
    for decision in [Decision::RevokeBake, Decision::Cancel, Decision::Approve] {
        let scratch = Scratch::new();
        let destination = scratch.0.join("output.ico");
        fs::write(&destination, OLD).unwrap();
        let expected = save::Expectation::acquired(&destination, OLD);
        let app = app();
        let original = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let config = paintcraft_ui::icon_export::Config::default();
        let target = export::metadata_policy::Target::new(Family::Ico, Operation::ExportAsset);
        let mut copy = export_options::AssetCopy::capture(&app).unwrap();
        copy.bake = true;
        copy.prepare(config.clone(), target).unwrap();
        copy.validate(&config, target).unwrap();
        assert!(copy.current(&app));
        let budget = paintcraft_runtime::SharedBudget::new(1 << 20);
        let owner = Owner {
            source: copy.frozen.document.clone(),
            _lease: budget.reserve(4096).unwrap(),
        };
        let job = WriteJob::start(
            NEXT.to_vec(),
            owner,
            destination.clone(),
            Some(expected),
            1024,
            &app.context,
        )
        .unwrap();
        let resolved = match event(&job) {
            WriteEvent::Authorize(resolved) => resolved.unwrap(),
            _ => panic!("write must stop at the final authorization boundary"),
        };
        assert_eq!(resolved, destination);
        assert_eq!(fs::read(&destination).unwrap(), OLD);
        assert_eq!(
            scratch.entries(),
            2,
            "real staged file still awaits approval"
        );
        let staging = fs::read_dir(&scratch.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p != &destination)
            .unwrap();
        assert_eq!(fs::read(&staging).unwrap(), NEXT);
        assert_eq!(
            budget.usage().reserved,
            4096,
            "bytes retain their admitted owner while waiting"
        );
        match decision {
            Decision::Cancel => {
                drop(job); // actual cancellation flag + authorization-channel retirement
                until(|| budget.usage().reserved == 0 && scratch.entries() == 1);
                assert_eq!(fs::read(&destination).unwrap(), OLD);
            }
            Decision::RevokeBake | Decision::Approve => {
                if matches!(decision, Decision::RevokeBake) {
                    copy.bake = false;
                }
                // The same frozen-intent/policy checks as the UI precede its answer.
                let valid = copy.current(&app) && copy.validate(&config, target).is_ok();
                assert_eq!(valid, matches!(decision, Decision::Approve));
                job.authorize(valid);
                let outcome = match event(&job) {
                    WriteEvent::Done(outcome) => outcome,
                    _ => panic!("authorization must produce an actual save outcome"),
                };
                assert_eq!(outcome.is_success(), valid);
                if valid {
                    assert_eq!(outcome.replacement, save::Replacement::Replaced);
                    assert_eq!(fs::read(&destination).unwrap(), NEXT);
                } else {
                    assert_eq!(outcome.replacement, save::Replacement::NotReplaced);
                    assert_eq!(fs::read(&destination).unwrap(), OLD);
                }
                assert_eq!(
                    scratch.entries(),
                    1,
                    "staging must be cleaned on either outcome"
                );
                assert_eq!(
                    budget.usage().reserved,
                    0,
                    "Done follows release of bytes and owner"
                );
            }
        }
        assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert!(!app.editor.is_dirty());
    }
}
