use super::*;

pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-test-{name}-{}",
        Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn phase_failures_preserve_truthful_replacement_outcomes() {
    for phase in [
        Phase::Encode,
        Phase::StageSync,
        Phase::Replace,
        Phase::ReadBack,
        Phase::DirectorySync,
    ] {
        let dir = scratch("phase");
        let dest = dir.join("a.pcnd");
        fs::write(&dest, b"original").unwrap();
        let out = save_impl(
            &dest,
            true,
            1024,
            |sink| sink.write_all(b"new").map_err(io_error),
            |current| {
                if current == phase {
                    Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
                } else {
                    Ok(())
                }
            },
        );
        let replaced = matches!(phase, Phase::ReadBack | Phase::DirectorySync);
        assert_eq!(
            out.replacement,
            if replaced {
                Replacement::Replaced
            } else {
                Replacement::NotReplaced
            }
        );
        assert_eq!(
            fs::read(&dest).unwrap(),
            if replaced {
                b"new".as_slice()
            } else {
                b"original".as_slice()
            }
        );
        assert!(!out.is_success());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn sink_quota_and_encoder_failures_cannot_publish_partial_files() {
    let dir = scratch("quota");
    let dest = dir.join("a.pcnd");
    fs::write(&dest, b"original").unwrap();
    let out = save_with(&dest, true, 2, |sink| {
        // Even an encoder incorrectly ignoring the failed write cannot publish.
        let _ = sink.write_all(b"longer than quota");
        Ok(())
    });
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert_eq!(fs::read(&dest).unwrap(), b"original");
    let out = save_with(&dest, true, 1024, |sink| {
        sink.write_all(b"partial").map_err(io_error)?;
        Err(Error::new(ErrorCode::CodecFailed, "injected encoder error"))
    });
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert_eq!(fs::read(&dest).unwrap(), b"original");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn changed_destination_after_replace_is_unknown_and_never_acknowledged() {
    let dir = scratch("external");
    let dest = dir.join("a.pcnd");
    let out = save_impl(
        &dest,
        true,
        1024,
        |sink| sink.write_all(b"ours").map_err(io_error),
        |phase| {
            if phase == Phase::ReadBack {
                fs::write(&dest, b"external")?;
            }
            Ok(())
        },
    );
    assert_eq!(out.replacement, Replacement::Unknown);
    let ctx = Context {
        operation: "save-as",
        source_state: Uuid::new_v4(),
        source_sha256: None,
        losses_acknowledged: vec![],
        destination: &dest,
    };
    let first = out.receipt_stamped(&ctx, 23, 4);
    assert_eq!(first, out.receipt_stamped(&ctx, 23, 4));
    assert_eq!(
        first.get("save_sequence").and_then(Value::as_str),
        Some("23")
    );
    assert_eq!(
        first.get("binding_generation").and_then(Value::as_str),
        Some("4")
    );
    assert_eq!(first.get("acknowledged_state_id"), Some(&Value::Null));
    assert_eq!(first.get("retry").and_then(Value::as_str), Some("inspect"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn altered_stage_is_rejected_before_replacement() {
    let dir = scratch("stage-corruption");
    let dest = dir.join("a.pcnd");
    fs::write(&dest, b"original").unwrap();
    let out = save_impl(
        &dest,
        true,
        1024,
        |sink| sink.write_all(b"encoded").map_err(io_error),
        |phase| {
            if phase == Phase::StageSync {
                for entry in fs::read_dir(&dir)? {
                    let path = entry?.path();
                    if path.extension().is_some_and(|e| e == "tmp") {
                        fs::write(path, b"corrupt")?;
                    }
                }
            }
            Ok(())
        },
    );
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert_eq!(fs::read(&dest).unwrap(), b"original");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_temporary_link_cleanup_cannot_report_non_replacement() {
    let dir = scratch("link-cleanup");
    let dest = dir.join("a.pcnd");
    let out = save_impl(
        &dest,
        false,
        1024,
        |sink| sink.write_all(b"new").map_err(io_error),
        |phase| {
            if phase == Phase::TemporaryCleanup {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(out.replacement, Replacement::Replaced);
    assert!(
        out.is_success(),
        "replacement and durability are independent of temporary-link cleanup"
    );
    assert_eq!(fs::read(&dest).unwrap(), b"new");
    assert!(out.reasons.iter().any(|r| r.contains("cleanup failed")));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn external_changes_during_encoding_are_not_overwritten() {
    let dir = scratch("fingerprint");
    for existed in [false, true] {
        let dest = dir.join(if existed { "existing.pcnd" } else { "new.pcnd" });
        if existed {
            fs::write(&dest, b"original").unwrap();
        }
        let out = save_with(&dest, true, 1024, |sink| {
            sink.write_all(b"ours").map_err(io_error)?;
            fs::write(&dest, b"external").map_err(io_error)
        });
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert_eq!(fs::read(&dest).unwrap(), b"external");
        assert!(
            out.reasons
                .iter()
                .any(|r| r.contains("changed while encoding"))
        );
    }
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        2,
        "failed requests remove staging"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_destinations_follow_captured_target_and_receipts_keep_its_identity() {
    use std::os::unix::fs::symlink;
    let dir = scratch("alias");
    let dest = dir.join("actual.pcnd");
    let alias = dir.join("alias.pcnd");
    fs::write(&dest, b"original").unwrap();
    symlink(&dest, &alias).unwrap();
    assert_eq!(destination_id(&alias), destination_id(&dest));
    let captured_id = destination_id(&dest);
    let out = save(&alias, b"new", true);
    assert!(out.is_success(), "{:?}", out.reasons);
    assert_eq!(fs::read(&dest).unwrap(), b"new");
    assert!(
        fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let other = dir.join("other.pcnd");
    fs::write(&other, b"other").unwrap();
    fs::remove_file(&alias).unwrap();
    symlink(&other, &alias).unwrap();
    let receipt = out.receipt(&Context {
        operation: "save",
        source_state: Uuid::new_v4(),
        source_sha256: None,
        losses_acknowledged: vec![],
        destination: &alias,
    });
    assert_eq!(
        receipt.get("destination_id").and_then(Value::as_str),
        Some(captured_id.as_str())
    );
    let dangling = dir.join("dangling.pcnd");
    symlink(dir.join("absent.pcnd"), &dangling).unwrap();
    assert_eq!(
        save(&dangling, b"new", true).replacement,
        Replacement::NotReplaced
    );
    assert!(
        fs::symlink_metadata(&dangling)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn parent_aliases_share_namespace_but_hardlinks_are_separate_replacement_paths() {
    use std::os::unix::fs::symlink;
    let dir = scratch("parent-alias");
    let actual = dir.join("actual");
    fs::create_dir(&actual).unwrap();
    symlink(&actual, dir.join("alias")).unwrap();
    let direct = actual.join("new.pcnd");
    let alias = dir.join("alias/new.pcnd");
    assert_eq!(destination_id(&direct), destination_id(&alias));
    assert!(save(&alias, b"original", false).is_success());
    let hardlink = actual.join("hardlink.pcnd");
    fs::hard_link(&direct, &hardlink).unwrap();
    assert_ne!(destination_id(&direct), destination_id(&hardlink));
    assert!(save(&hardlink, b"replacement", true).is_success());
    assert_eq!(fs::read(&direct).unwrap(), b"original");
    assert_eq!(fs::read(&hardlink).unwrap(), b"replacement");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn creates_new_files_and_leaves_no_staging_files() {
    let dir = scratch("create");
    let dest = dir.join("a.pcnd");
    let out = save(&dest, b"hello", false);
    assert!(out.is_success(), "{:?}", out.reasons);
    assert_eq!(fs::read(&dest).unwrap(), b"hello");
    assert_eq!(out.observed_sha256, out.staged_sha256);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn refuses_to_clobber_without_overwrite_and_keeps_the_original() {
    let dir = scratch("clobber");
    let dest = dir.join("a.pcnd");
    fs::write(&dest, b"original").unwrap();
    assert_eq!(
        preflight(&dest, false).unwrap_err().code,
        ErrorCode::TargetInvalid
    );
    let out = save(&dest, b"new", false);
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert_eq!(fs::read(&dest).unwrap(), b"original");
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        1,
        "staging file removed"
    );
    let out = save(&dest, b"new", true);
    assert!(out.is_success());
    assert_eq!(fs::read(&dest).unwrap(), b"new");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_parent_is_a_known_non_replacement() {
    let dir = scratch("missing");
    let out = save(&dir.join("no/such/dir/a.pcnd"), b"x", true);
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert!(!out.is_success() && !out.reasons.is_empty());
    let ctx = Context {
        operation: "save-as",
        source_state: Uuid::nil(),
        source_sha256: None,
        losses_acknowledged: vec![],
        destination: &dir,
    };
    let receipt = out.receipt(&ctx);
    assert_eq!(
        receipt.get("retry").and_then(Value::as_str),
        Some("allowed-new-request")
    );
    assert_eq!(receipt.get("acknowledged_state_id"), Some(&Value::Null));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn directories_and_receipt_shape() {
    let dir = scratch("dir");
    assert_eq!(
        preflight(&dir, true).unwrap_err().code,
        ErrorCode::TargetInvalid
    );
    let dest = dir.join("a.pcnd");
    let out = save(&dest, b"abc", false);
    let ctx = Context {
        operation: "save-as",
        source_state: Uuid::nil(),
        source_sha256: Some("a".repeat(64)),
        losses_acknowledged: vec![],
        destination: &dest,
    };
    let r = out.receipt(&ctx);
    for key in ["replacement", "durability", "retry"] {
        assert!(r.get(key).and_then(Value::as_str).is_some(), "{key}");
    }
    assert_eq!(
        r.get("replacement").and_then(Value::as_str),
        Some("replaced")
    );
    assert_eq!(
        r.get("durability").and_then(Value::as_str),
        Some("confirmed")
    );
    assert!(destination_id(&dest).starts_with("dest:") && !destination_id(&dest).contains('/'));
    fs::remove_dir_all(dir).unwrap();
}
