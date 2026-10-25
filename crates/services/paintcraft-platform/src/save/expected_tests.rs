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
