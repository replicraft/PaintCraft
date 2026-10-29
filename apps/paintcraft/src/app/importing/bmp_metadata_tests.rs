//! Native new-document BMP retention uses real consent, source and host quotas.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_document::source_metadata::{
    BYTES_MAX, ResolutionUnit, SourceColorModel, SourceFormat, WIRE_HEADER_BYTES, WorkingColor,
};
use std::fs;

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/bmp-source-retention/native")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests/bmp-retention-fixtures")
            .join(name),
    )
    .unwrap()
}
fn pixels(document: &paintcraft_document::Document) -> Vec<u8> {
    let raster = &document.layers[&document.roots[0]].raster;
    let mut out = Vec::new();
    for y in 0..document.canvas.1 {
        for x in 0..document.canvas.0 {
            out.extend(raster.get(i64::from(x), i64::from(y)).0);
        }
    }
    out
}
#[test]
fn native_bmp_exact_original_profile_signed_density_and_pixels_survive_consent_and_pcnd() {
    let cases = [
        ("rgb24-v40-density-positive.bmp", [3780, 3779], false),
        ("rgb24-v40-density-mixed-zero.bmp", [0, -3780], false),
        ("rgb24-v108-density-negative.bmp", [-3780, -3779], false),
        (
            "rgb24-v124-density-extremes.bmp",
            [i32::MIN, i32::MAX],
            false,
        ),
        ("rgb24-v124-profile-only.bmp", [0, 0], true),
        ("rgb24-v124-profile-positive.bmp", [3780, 3779], true),
        ("rgb24-v124-profile-negative.bmp", [-3780, 0], true),
        ("rgba32-v124-profile-hidden.bmp", [0, 3779], true),
        ("indexed1-v124-profile-reserved.bmp", [0, 0], true),
        ("indexed4-v124-profile-reserved.bmp", [0, 0], true),
        ("indexed8-v124-profile-reserved.bmp", [0, 0], true),
    ];
    let profile = include_bytes!("../../../../../assets/profiles/sRGB2014.icc");
    for experience in [Experience::Classic, Experience::Studio] {
        for (name, density, has_profile) in cases {
            let folder = Folder::new();
            let path = folder.0.join(name);
            let original = fixture(name);
            fs::write(&path, &original).unwrap();
            let expected = fixture(&format!("{name}.rgba"));
            let mut app = App::new(None);
            app.settings.experience = experience;
            let realm = SharedBudget::new(1 << 20);
            app.source_metadata_budget = realm.clone();
            let before = app.editor.document_snapshot();
            app.open_path(&path).unwrap();
            let pending = app.pending_import.as_ref().unwrap();
            let metadata = pending.source_metadata.as_ref().unwrap().clone();
            let facts = metadata.descriptor();
            assert_eq!(facts.format, SourceFormat::Bmp);
            assert_eq!(
                facts.source_sha256,
                <[u8; 32]>::from(Sha256::digest(&original))
            );
            assert_eq!(facts.source_bytes, original.len() as u64);
            assert_eq!(facts.source_dimensions, (3, 2));
            assert_eq!(facts.output_dimensions, (3, 2));
            assert_eq!(facts.orientation, None);
            assert_eq!(
                facts.source_model,
                if name.starts_with("indexed") {
                    SourceColorModel::Indexed
                } else {
                    SourceColorModel::Rgb
                }
            );
            assert_eq!(metadata.working_color(), WorkingColor::UnmanagedRgba8);
            assert_eq!(
                metadata.profile(),
                has_profile.then_some(profile.as_slice())
            );
            if density == [0, 0] {
                assert!(facts.resolution.is_none());
            } else {
                let resolution = facts.resolution.unwrap();
                assert_eq!(resolution.unit, ResolutionUnit::Metre);
                assert_eq!(resolution.raw_unit, None);
                assert_eq!(resolution.horizontal.numerator, i64::from(density[0]));
                assert_eq!(resolution.vertical.numerator, i64::from(density[1]));
                assert_eq!(resolution.horizontal.denominator, 1);
                assert_eq!(resolution.vertical.denominator, 1);
                assert_eq!(
                    resolution.physical().is_some(),
                    density.iter().all(|n| *n > 0)
                );
            }
            let Candidate::Raster(image) = &pending.candidate else {
                panic!("raster required")
            };
            assert_eq!(image.rgba, expected);
            let expected_state = import_image(
                image.width,
                image.height,
                &image.rgba,
                &pending.raster_key.as_ref().unwrap().hex(),
            )
            .unwrap()
            .with_source_metadata_checked(Some(metadata.clone()), &|| Ok(()))
            .unwrap()
            .state_id;
            assert_eq!(
                image
                    .losses
                    .iter()
                    .any(|s| s == image::bmp_retention::ICC_RETAINED_UNAPPLIED),
                has_profile
            );
            assert_eq!(
                image
                    .losses
                    .iter()
                    .any(|s| s == image::bmp_retention::RESOLUTION_RETAINED_INACTIVE),
                density != [0, 0]
            );
            assert!(
                !image
                    .losses
                    .iter()
                    .any(|s| s == "icc-profile-ignored" || s == "resolution-metadata-ignored")
            );
            // A direct request cannot replace the visible preview and consent.
            app.resolve_import(&app.context.clone(), true);
            assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
            assert!(app.pending_import.is_some());
            let ctx = super::raster_review::tests::authorize(&mut app);
            app.resolve_import(&ctx, true);
            assert!(app.pending_import.is_none());
            assert!(app.editor.is_dirty());
            assert_eq!(app.editor.document().state_id, expected_state);
            assert!(app.bound.is_none());
            assert_eq!(app.imported_source.as_ref(), Some(&path));
            assert_eq!(pixels(app.editor.document()), expected);
            assert_eq!(
                app.editor.document().source_metadata.as_ref(),
                Some(&metadata)
            );
            let copy = paintcraft_formats::pcnd::write_compact(app.editor.document()).unwrap();
            let reopened = app.read_project_with_metadata(&copy).unwrap();
            assert_eq!(reopened.source_metadata.as_ref(), Some(&metadata));
            assert_eq!(pixels(&reopened), expected);
            assert_eq!(reopened.state_id, app.editor.document().state_id);
            assert_eq!(realm.usage().reserved, metadata.storage_usage().1 * 2);
            let alias = folder.0.join("source-alias.pcnd");
            fs::hard_link(&path, &alias).unwrap();
            assert!(app.guard_save_destination(&alias).is_err());
            assert!(app.guard_export_destination(&alias).is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
            drop(reopened);
            drop(metadata);
            drop(app);
            assert_eq!(realm.usage().reserved, 0);
        }
    }
}

#[test]
fn native_bmp_cancel_stale_source_replacement_and_exhausted_realm_keep_existing_work() {
    for mode in 0..5 {
        let folder = Folder::new();
        let path = folder.0.join("source.bmp");
        let original = fixture("rgb24-v124-profile-negative.bmp");
        fs::write(&path, &original).unwrap();
        let mut app = App::new(None);
        app.editor.request(
            780,
            Action::ClearCanvas {
                color: Rgba([9, 10, 11, 255]),
            },
        );
        app.editor.run_pending();
        let before = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        let realm = SharedBudget::new(if mode == 4 { 0 } else { 1 << 20 });
        app.source_metadata_budget = realm.clone();
        let opened = app.open_path(&path);
        if mode == 4 {
            assert_eq!(opened.unwrap_err().code, ErrorCode::ResourceLimit);
        } else {
            opened.unwrap();
            assert!(realm.usage().reserved > 0);
            let ctx = super::raster_review::tests::authorize(&mut app);
            match mode {
                1 => app.settings.classic_variant = Some(ClassicVariant::V7),
                2 => {
                    fs::write(&path, b"changed source").unwrap();
                }
                3 => {
                    fs::rename(&path, folder.0.join("original.bmp")).unwrap();
                    fs::write(&path, &original).unwrap();
                }
                _ => {}
            }
            app.resolve_import(&ctx, mode != 0);
        }
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(history, app.editor.history_view().ticket);
        assert_eq!(dirty, app.editor.is_dirty());
        assert!(app.pending_import.is_none());
        assert!(app.bound.is_none());
        assert!(app.imported_source.is_none());
        assert!(app.auxiliary_sources.is_empty());
        assert_eq!(realm.usage().reserved, 0);
    }
}

#[test]
fn zero_density_without_profile_needs_no_metadata_lease_and_legacy_decode_still_omits() {
    let folder = Folder::new();
    let path = folder.0.join("empty.bmp");
    let bytes = fixture("rgb24-v40-no-metadata.bmp");
    fs::write(&path, &bytes).unwrap();
    let mut app = App::new(None);
    let realm = SharedBudget::new(0);
    app.source_metadata_budget = realm.clone();
    app.open_path(&path).unwrap();
    assert!(app.pending_import.is_none());
    assert!(app.editor.document().source_metadata.is_none());
    assert_eq!(
        pixels(app.editor.document()),
        fixture("rgb24-v40-no-metadata.bmp.rgba")
    );
    assert_eq!(realm.usage().reserved, 0);
    let prepared = image::bmp_retention::decode(
        &bytes,
        image::DECODE_PIXELS_MAX,
        paintcraft_formats::pcnd::MetadataAdmission {
            bytes: BYTES_MAX,
            budget: &realm,
        },
        &|| Ok(()),
    )
    .unwrap();
    let expected = import_image(3, 2, &prepared.image.rgba, &prepared.document_key.hex()).unwrap();
    let raw_identity =
        import_image(3, 2, &prepared.image.rgba, &hex(&Sha256::digest(&bytes))).unwrap();
    assert_eq!(app.editor.document().state_id, expected.state_id);
    assert_ne!(app.editor.document().state_id, raw_identity.state_id);
    // Calibrated RGB declarations remain an explicit omission, not an ICC.
    let name = "rgb24-v108-calibrated-no-retention.bmp";
    let path = folder.0.join(name);
    fs::write(&path, fixture(name)).unwrap();
    let mut calibrated = App::new(None);
    let realm = SharedBudget::new(1 << 20);
    calibrated.source_metadata_budget = realm.clone();
    calibrated.open_path(&path).unwrap();
    assert!(
        calibrated
            .pending_import
            .as_ref()
            .unwrap()
            .source_metadata
            .is_none()
    );
    let ctx = super::raster_review::tests::authorize(&mut calibrated);
    calibrated.resolve_import(&ctx, true);
    assert!(calibrated.editor.is_dirty());
    assert!(calibrated.editor.document().source_metadata.is_none());
    assert_eq!(
        pixels(calibrated.editor.document()),
        fixture(&format!("{name}.rgba"))
    );
    assert_eq!(realm.usage().reserved, 0);
    let legacy = image::decode(&fixture("rgb24-v124-profile-negative.bmp")).unwrap();
    assert!(legacy.losses.iter().any(|s| s == "icc-profile-ignored"));
    assert!(
        legacy
            .losses
            .iter()
            .any(|s| s == "resolution-metadata-ignored")
    );
}

#[test]
fn initial_bmp_adoption_refuses_unsavable_metadata_without_replacing_the_document() {
    let folder = Folder::new();
    let path = folder.0.join("near-limit.bmp");
    let mut bytes = fixture("rgb24-v124-profile-positive.bmp");
    let start = 14 + u32::from_le_bytes(bytes[126..130].try_into().unwrap()) as usize;
    let profile_bytes = BYTES_MAX as usize - WIRE_HEADER_BYTES;
    bytes.resize(start + profile_bytes, 0);
    bytes[start..start + 4].copy_from_slice(&(profile_bytes as u32).to_be_bytes());
    bytes[130..134].copy_from_slice(&(profile_bytes as u32).to_le_bytes());
    let len = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&len.to_le_bytes());
    fs::write(&path, &bytes).unwrap();
    let mut app = App::new(None);
    let realm = SharedBudget::new(BYTES_MAX + (1 << 20));
    app.source_metadata_budget = realm.clone();
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    app.open_path(&path).unwrap();
    assert_eq!(
        app.pending_import
            .as_ref()
            .unwrap()
            .source_metadata
            .as_ref()
            .unwrap()
            .profile()
            .unwrap()
            .len(),
        profile_bytes
    );
    let ctx = super::raster_review::tests::authorize(&mut app);
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(ticket, app.editor.history_view().ticket);
    assert!(app.pending_import.is_none());
    assert!(app.editor.document().source_metadata.is_none());
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
