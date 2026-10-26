use super::*;
use std::{cell::Cell, fs};
fn document() -> Document {
    paintcraft_formats::project_import::decode_ora(
        include_bytes!(
            "../../../../../crates/services/paintcraft-formats/tests/ora-fixtures/deflated.ora"
        ),
        100,
        &|| Ok(()),
    )
    .unwrap()
    .document
}
fn folder() -> PathBuf {
    let path = std::env::temp_dir().join(format!("paintcraft-layered-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path).unwrap();
    path
}
#[test]
fn post_admission_copy_retains_layers_and_does_not_mutate_source() {
    let folder = folder();
    let doc = document();
    let before = pcnd::write(&doc).unwrap();
    for format in [Format::Psd, Format::Psb] {
        let dest = folder.join(format!("copy.{}", format.family().id()));
        let (_, ok, summary, _) = write_copy(
            &doc,
            format.target(None, None).unwrap(),
            &dest,
            false,
            &[],
            None,
            &|| Ok(()),
        )
        .unwrap();
        assert!(ok, "{summary}");
        let decoded = psd::decode(&fs::read(dest).unwrap(), psd::PIXELS_MAX, &|| Ok(())).unwrap();
        for (actual, id) in decoded.layers.iter().zip(&doc.roots) {
            let source = &doc.layers[id];
            assert_eq!(actual.info.name, source.name);
            assert_eq!(actual.info.opacity, source.opacity);
            assert_eq!(actual.info.visible, source.visible);
            assert_eq!(
                actual.info.origin,
                (source.origin.0 as i32, source.origin.1 as i32)
            );
            for (i, pixel) in actual.rgba.as_chunks::<4>().0.iter().enumerate() {
                assert_eq!(
                    *pixel,
                    source
                        .raster
                        .get(
                            (i % actual.info.width as usize) as i64,
                            (i / actual.info.width as usize) as i64
                        )
                        .0
                );
            }
        }
    }
    assert_eq!(pcnd::write(&doc).unwrap(), before);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn consent_and_guard_failures_preserve_output_and_remove_private_stage() {
    let folder = folder();
    let dest = folder.join("copy.psd");
    fs::write(&dest, b"original").unwrap();
    let mut doc = document();
    doc.title = Some("native title".into());
    assert!(
        write_copy(
            &doc,
            Target::Psd(psd::Version::Psd),
            &dest,
            true,
            &[],
            None,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        write_copy(
            &doc,
            Target::Psd(psd::Version::Psd),
            &dest,
            true,
            &["unknown".into()],
            None,
            &|| Ok(())
        )
        .is_err()
    );
    let allowed = vec![psd::TITLE_LOSS.into()];
    let count = Cell::new(0usize);
    let (_, ok, _, _) = write_copy(
        &doc,
        Target::Psd(psd::Version::Psd),
        &dest,
        true,
        &allowed,
        None,
        &|| {
            count.set(count.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    assert!(ok);
    let checkpoints = count.get();
    for stop in 1..=checkpoints {
        fs::write(&dest, b"original").unwrap();
        let count = Cell::new(0usize);
        let _ = write_copy(
            &doc,
            Target::Psd(psd::Version::Psd),
            &dest,
            true,
            &allowed,
            None,
            &|| {
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(fs::read(&dest).unwrap(), b"original", "checkpoint {stop}");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
    }
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn every_profile_requires_qualification_and_preserving_save_stays_denied() {
    for frontend in [
        Frontend::Studio,
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        for format in [Format::Psd, Format::Psb, Format::Pdn] {
            let path = PathBuf::from(format!("copy.{}", format.family().id()));
            assert!(admit(frontend, format, &path).is_err());
            assert!(
                registry::admission(Request::new(
                    frontend,
                    Operation::SavePreserving,
                    format.family(),
                    Platform::Native
                ))
                .require_allowed(Request::new(
                    frontend,
                    Operation::SavePreserving,
                    format.family(),
                    Platform::Native
                ))
                .is_err()
            );
        }
    }
    assert_eq!(
        admit(Frontend::Studio, Format::Psd, Path::new("copy.psb"))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn captured_source_renames_and_hardlink_destinations_stay_protected() {
    let folder = folder();
    let source = folder.join("source.pcnd");
    let renamed = folder.join("renamed.pcnd");
    let doc = document();
    let original = pcnd::write(&doc).unwrap();
    fs::write(&source, &original).unwrap();
    let (_, _, identity) = read_captured(&source, INPUT_MAX, "test source").unwrap();
    fs::rename(&source, &renamed).unwrap();
    let alias = folder.join("alias.psd");
    fs::hard_link(&renamed, &alias).unwrap();
    for path in [&renamed, &alias] {
        let guard = || {
            if identity.matches_path(path)? {
                Err(Error::new(ErrorCode::InputSchema, "captured source"))
            } else {
                Ok(())
            }
        };
        assert!(
            write_copy(
                &doc,
                Target::Psd(psd::Version::Psd),
                path,
                true,
                &[],
                None,
                &guard
            )
            .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), original);
    }
    let dest = folder.join("late.psd");
    fs::write(&dest, b"original").unwrap();
    // Simulate a source alias introduced after the initial preflight. The staged
    // service must not overwrite that late destination even when overwrite=true.
    let baseline = Cell::new(0);
    psd::preflight(
        &doc,
        psd::Version::Psd,
        &psd::WriteLimits::default(),
        &|| {
            baseline.set(baseline.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    let calls = Cell::new(0);
    let guard = || {
        calls.set(calls.get() + 1);
        if calls.get() == baseline.get() + 1 {
            fs::remove_file(&dest).unwrap();
            fs::hard_link(&renamed, &dest).unwrap();
        }
        if identity.matches_path(&dest)? {
            Err(Error::new(ErrorCode::InputSchema, "late source alias"))
        } else {
            Ok(())
        }
    };
    let _ = write_copy(
        &doc,
        Target::Psd(psd::Version::Psd),
        &dest,
        true,
        &[],
        None,
        &guard,
    );
    assert_eq!(fs::read(&dest).unwrap(), original);
    assert_eq!(fs::read(&renamed).unwrap(), original);
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 3);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn pdn_staged_copy_records_explicit_policy_and_keeps_hidden_rgb_and_source_state() {
    let folder = folder();
    let dest = folder.join("copy.pdn");
    let mut doc = new_document(2, 2, Some(Rgba([10, 20, 30, 0])), 3, POLICY).unwrap();
    doc.title = Some("source title".into());
    let before = pcnd::write(&doc).unwrap();
    let target = Format::Pdn
        .target(Some(Background::Ordinary), Some(Geometry::Exact))
        .unwrap();
    assert!(write_copy(&doc, target, &dest, false, &[], None, &|| Ok(())).is_err());
    assert!(!dest.exists());
    let allowed = vec![pdn::TITLE_LOSS.into()];
    let (receipt, ok, summary, preparation) =
        write_copy(&doc, target, &dest, false, &allowed, None, &|| Ok(())).unwrap();
    assert!(ok, "{summary}");
    assert_eq!(
        preparation
            .unwrap()
            .get("background_role")
            .unwrap()
            .as_str(),
        Some("ordinary")
    );
    assert_eq!(
        receipt.get("losses_acknowledged"),
        Some(&Value::Array(vec![Value::str(pdn::TITLE_LOSS)]))
    );
    let bytes = fs::read(&dest).unwrap();
    let decoded = paintcraft_formats::project_import::decode_pdn(&bytes, 100, &|| Ok(())).unwrap();
    let layer = &decoded.document.layers[&decoded.document.roots[0]];
    for y in 0..2 {
        for x in 0..2 {
            assert_eq!(layer.raster.get(x, y), Rgba([10, 20, 30, 0]));
        }
    }
    assert_eq!(pcnd::write(&doc).unwrap(), before);
    let count = Cell::new(0);
    write_copy(&doc, target, &dest, true, &allowed, None, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        fs::write(&dest, b"original").unwrap();
        let calls = Cell::new(0);
        let _ = write_copy(&doc, target, &dest, true, &allowed, None, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        });
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
    }
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn pdn_roles_are_explicit_and_pdn_only_options_do_not_apply_to_psd() {
    assert!(Format::Pdn.target(None, None).is_err());
    assert!(
        Format::Psd
            .target(Some(Background::Ordinary), None)
            .is_err()
    );
    assert!(
        Format::Psb
            .target(None, Some(Geometry::Materialize))
            .is_err()
    );
}
