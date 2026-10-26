use super::*;
use std::cell::Cell;
fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[test]
fn captured_content_is_required_before_encoding_or_staging() {
    let dir = tests::scratch("expected-content");
    let dest = dir.join("image.pcnd");
    let expected = digest(b"old bytes");
    for newer in [
        None,
        Some(b"new bytes".as_slice()),
        Some(b"other".as_slice()),
    ] {
        if let Some(bytes) = newer {
            fs::write(&dest, bytes).unwrap();
        }
        let called = Cell::new(false);
        let out = save_with_expected_sha256(&dest, true, 1024, &expected, |_| {
            called.set(true);
            Ok(())
        });
        assert!(!called.get());
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert!(!out.is_success());
        assert!(out.staged_sha256.is_none());
        assert!(
            out.reasons
                .iter()
                .any(|r| r.contains("changed since input capture"))
        );
        assert_eq!(fs::read(&dest).ok().as_deref(), newer);
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            usize::from(newer.is_some())
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn matching_content_saves_but_a_later_external_change_still_survives() {
    let dir = tests::scratch("expected-late-change");
    let dest = dir.join("image.pcnd");
    fs::write(&dest, b"old").unwrap();
    let out = save_with_expected_sha256(&dest, true, 1024, &digest(b"old"), |sink| {
        sink.write_all(b"saved").map_err(io_error)
    });
    assert!(out.is_success());
    assert_eq!(fs::read(&dest).unwrap(), b"saved");
    let out = save_with_expected_sha256(&dest, true, 1024, &digest(b"saved"), |sink| {
        fs::write(&dest, b"external").map_err(io_error)?;
        sink.write_all(b"staged").map_err(io_error)
    });
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert!(!out.is_success());
    assert_eq!(
        out.staged_sha256.as_deref(),
        Some(digest(b"staged").as_str())
    );
    assert_eq!(fs::read(&dest).unwrap(), b"external");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_expectations_do_not_touch_destinations_or_invoke_encoders() {
    for expected in ["", "a", &"A".repeat(64), &"z".repeat(64)] {
        let out = save_with_expected_sha256(
            Path::new("unreachable/without-a-directory"),
            true,
            1024,
            expected,
            |_| panic!("must not encode"),
        );
        assert!(!out.is_success());
        assert!(out.destination_id.is_none());
        assert!(
            out.reasons
                .iter()
                .any(|r| r.contains("lowercase hexadecimal"))
        );
    }
}

#[test]
fn namespace_guard_rejects_equal_content_in_a_different_destination_before_staging() {
    let dir = tests::scratch("expected-namespace");
    let a = dir.join("a.pcnd");
    let b = dir.join("b.pcnd");
    fs::write(&a, b"same bytes").unwrap();
    fs::write(&b, b"same bytes").unwrap();
    let a = fs::canonicalize(a).unwrap();
    let expected = Expectation::acquired(&a, b"same bytes");
    let out = save_with_expectation_and_preparation(
        &b,
        true,
        1024,
        Some(&expected),
        |_| panic!("must not encode a different namespace"),
        |_| panic!("must not prepare"),
    );
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert!(out.reasons.iter().any(|r| r.contains("namespace changed")));
    assert_eq!(fs::read(&b).unwrap(), b"same bytes");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
    let mut prepared = false;
    let out = save_with_expectation_and_preparation(
        &a,
        true,
        1024,
        Some(&expected),
        |sink| sink.write_all(b"saved").map_err(io_error),
        |outcome| {
            assert_eq!(
                outcome.staged_sha256.as_deref(),
                Some(digest(b"saved").as_str())
            );
            prepared = true;
            Ok(())
        },
    );
    assert!(prepared && out.is_success());
    let next = out.written_expectation().unwrap();
    assert_eq!(next.sha256, digest(b"saved"));
    assert_eq!(next.destination_id, expected.destination_id);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expected_content_and_failed_preparation_preserve_staged_outcome_and_destination() {
    let dir = tests::scratch("expected-content-preparation");
    let dest = dir.join("image.pcnd");
    fs::write(&dest, b"old bytes").unwrap();
    let out = save_with_expected_sha256_and_preparation(
        &dest,
        true,
        1024,
        &digest(b"old bytes"),
        |sink| sink.write_all(b"new bytes").map_err(io_error),
        |staged| {
            assert_eq!(
                staged.staged_sha256.as_deref(),
                Some(digest(b"new bytes").as_str())
            );
            assert_eq!(staged.replacement, Replacement::NotReplaced);
            Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "source guard refused",
            ))
        },
    );
    assert_eq!(out.replacement, Replacement::NotReplaced);
    assert!(!out.is_success());
    assert_eq!(out.staged_sha256, Some(digest(b"new bytes")));
    assert_eq!(fs::read(&dest).unwrap(), b"old bytes");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}
