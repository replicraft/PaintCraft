use super::*;
use paintcraft_core::types::{json, policy::hex};
use sha2::{Digest, Sha256};
use std::{fs, io::Write};
const MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V1\0";
const RECORD:&[u8]=b"PAINTCRAFT_CODEC_WORKER_BUILD_V1\0\x30.1.0\0PCDEC002\0HEAP268435456\0END_CODEC_WORKER_BUILD\0";
fn contract() -> Contract<'static> {
    Contract {
        application_version: "0.1.0",
        protocol_version: 2,
        profile: "internal-heif-gray8-jxl-rgba8",
        requested_heap_ceiling_bytes: 256 << 20,
        qualified_import_variants: &["qualified-test-reader"],
        build_marker: MARKER,
        build_record: RECORD,
    }
}
fn binary(target: Target) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    match target {
        Target::MacArm | Target::MacX64 => {
            bytes[..4].copy_from_slice(&0xfeedfacfu32.to_le_bytes());
            bytes[4..8].copy_from_slice(
                &if matches!(target, Target::MacArm) {
                    0x0100000cu32
                } else {
                    0x01000007u32
                }
                .to_le_bytes(),
            );
            bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        }
        Target::WinArm | Target::WinX64 => {
            bytes[..2].copy_from_slice(b"MZ");
            bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
            bytes[64..68].copy_from_slice(b"PE\0\0");
            bytes[68..70].copy_from_slice(
                &if matches!(target, Target::WinArm) {
                    0xaa64u16
                } else {
                    0x8664u16
                }
                .to_le_bytes(),
            );
            bytes[86..88].copy_from_slice(&2u16.to_le_bytes());
            bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
        }
        Target::LinuxArm | Target::LinuxX64 => {
            bytes[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
            bytes[16..18].copy_from_slice(&3u16.to_le_bytes());
            bytes[18..20].copy_from_slice(
                &if matches!(target, Target::LinuxArm) {
                    183u16
                } else {
                    62u16
                }
                .to_le_bytes(),
            );
        }
    }
    bytes.extend(RECORD);
    bytes
}
struct Installation {
    root: PathBuf,
    app: PathBuf,
    helper: PathBuf,
    manifest: PathBuf,
    target: Target,
}
impl Installation {
    fn new(target: Target, linux: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "paintcraft-installed-helper-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let directory = if linux {
            root.join("prefix/bin")
        } else {
            root.join("portable with spaces")
        };
        fs::create_dir_all(&directory).unwrap();
        let app = directory.join("paintcraft");
        let helper = directory.join(target.executable());
        let manifest = if linux {
            root.join("prefix/share/paintcraft").join(MANIFEST_NAME)
        } else {
            directory.join(MANIFEST_NAME)
        };
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&app, binary(target)).unwrap();
        fs::write(&helper, binary(target)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let result = Self {
            root,
            app,
            helper,
            manifest,
            target,
        };
        result.refresh();
        result
    }
    fn metadata(&self) -> Value {
        Value::object([
            ("schema_version", Value::Int(2)),
            ("embedded_build_record_verified", Value::Bool(true)),
            ("application_version", Value::str("0.1.0")),
            ("target", Value::str(self.target.triple())),
            ("executable", Value::str(self.target.executable())),
            (
                "sha256",
                Value::str(hex(&Sha256::digest(fs::read(&self.helper).unwrap()))),
            ),
            ("protocol_version", Value::Int(2)),
            ("profile", Value::str("internal-heif-gray8-jxl-rgba8")),
            ("requested_heap_ceiling_bytes", Value::Int(256 << 20)),
            (
                "qualified_import_variants",
                Value::Array(vec![Value::str("qualified-test-reader")]),
            ),
        ])
    }
    fn refresh(&self) {
        fs::write(&self.manifest, json::canonical(&self.metadata())).unwrap();
    }
    fn verify(&self) -> Result<VerifiedHelper> {
        verify(&self.app, contract(), self.target, &|| Ok(()))
    }
}
impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn all_release_targets_and_adjacent_or_linux_share_layouts_validate_actual_headers() {
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::WinArm,
        Target::WinX64,
        Target::LinuxArm,
        Target::LinuxX64,
    ] {
        for linux in [false, true]
            .into_iter()
            .filter(|linux| !*linux || target.linux())
        {
            let f = Installation::new(target, linux);
            let verified = f.verify().unwrap();
            assert_eq!(verified.checked_executable(&|| Ok(())).unwrap(), f.helper);
            assert_eq!(
                verified.sha256(),
                <[u8; 32]>::from(Sha256::digest(fs::read(&f.helper).unwrap()))
            );
        }
    }
}
#[test]
fn missing_stale_wrong_architecture_and_changed_records_fail_before_launch() {
    let f = Installation::new(Target::MacArm, false);
    for bad in [
        binary(Target::MacX64),
        binary(Target::MacArm)
            .into_iter()
            .map(|b| if b == b'H' { b'Q' } else { b })
            .collect(),
        vec![0; 128],
    ] {
        fs::write(&f.helper, bad).unwrap();
        f.refresh();
        assert!(f.verify().is_err());
    }
    for (from, to) in [
        (b"0.1.0".as_slice(), b"0.2.0".as_slice()),
        (b"PCDEC002", b"PCDEC003"),
        (b"268435456", b"268435457"),
    ] {
        let mut bytes = binary(f.target);
        let index = bytes
            .windows(from.len())
            .position(|part| part == from)
            .unwrap();
        bytes[index..index + to.len()].copy_from_slice(to);
        fs::write(&f.helper, bytes).unwrap();
        f.refresh();
        assert!(f.verify().is_err());
    }
    fs::remove_file(&f.helper).unwrap();
    assert!(f.verify().is_err());
}
#[test]
fn strict_manifest_fields_digest_duplicates_and_quotas_are_enforced() {
    let f = Installation::new(Target::LinuxX64, true);
    for (key, value) in [
        ("application_version", Value::str("0.2.0")),
        ("protocol_version", Value::Int(3)),
        (
            "qualified_import_variants",
            Value::Array(vec![Value::str("foreign-reader")]),
        ),
        ("target", Value::str("aarch64-unknown-linux-gnu")),
        ("sha256", Value::str("0".repeat(64))),
    ] {
        let Value::Object(mut fields) = f.metadata() else {
            panic!()
        };
        fields.insert(key.into(), value);
        fs::write(&f.manifest, json::canonical(&Value::Object(fields))).unwrap();
        assert!(f.verify().is_err());
    }
    let mut duplicate = json::canonical(&f.metadata());
    duplicate.pop();
    duplicate.extend(b",\"schema_version\":1}");
    fs::write(&f.manifest, duplicate).unwrap();
    assert!(f.verify().is_err());
    fs::write(&f.manifest, vec![b' '; 8193]).unwrap();
    assert_eq!(f.verify().unwrap_err().code, ErrorCode::ResourceLimit);
}
#[test]
fn acquired_identity_and_exact_read_bytes_are_rechecked_before_returning_launch_path() {
    let f = Installation::new(Target::MacArm, false);
    let verified = f.verify().unwrap();
    let mut file = fs::OpenOptions::new().write(true).open(&f.helper).unwrap();
    file.write_all(b"changed").unwrap();
    drop(file);
    assert!(verified.checked_executable(&|| Ok(())).is_err());
    fs::write(&f.helper, binary(f.target)).unwrap();
    f.refresh();
    let verified = f.verify().unwrap();
    fs::rename(&f.helper, f.root.join("captured-original")).unwrap();
    fs::write(&f.helper, binary(f.target)).unwrap();
    assert!(verified.checked_executable(&|| Ok(())).is_err());
}
#[test]
fn manifest_replacement_and_ambiguous_linux_adjacent_manifest_fail_closed() {
    let f = Installation::new(Target::LinuxX64, true);
    let verified = f.verify().unwrap();
    fs::write(&f.manifest, b"{}").unwrap();
    assert!(verified.checked_executable(&|| Ok(())).is_err());
    f.refresh();
    fs::write(f.app.parent().unwrap().join(MANIFEST_NAME), b"{}").unwrap();
    assert!(f.verify().is_err());
}
#[test]
fn cancellation_and_stream_boundary_build_records_return_no_partial_guard() {
    let f = Installation::new(Target::LinuxX64, false);
    let mut bytes = binary(f.target);
    bytes.truncate(128);
    bytes.resize(65536 - 15, 0);
    bytes.extend(RECORD);
    fs::write(&f.helper, bytes).unwrap();
    f.refresh();
    assert!(f.verify().is_ok());
    let count = std::cell::Cell::new(0);
    let result = verify(&f.app, contract(), f.target, &|| {
        let n = count.get();
        count.set(n + 1);
        if n > 3 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel verification",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
}
#[cfg(unix)]
#[test]
fn symlinked_helper_manifest_and_directory_are_refused() {
    use std::os::unix::fs::symlink;
    let f = Installation::new(Target::MacArm, false);
    let copy = f.root.join("helper-copy");
    fs::rename(&f.helper, &copy).unwrap();
    symlink(&copy, &f.helper).unwrap();
    assert!(f.verify().is_err());
    fs::remove_file(&f.helper).unwrap();
    fs::rename(&copy, &f.helper).unwrap();
    let metadata = f.root.join("manifest-copy");
    fs::rename(&f.manifest, &metadata).unwrap();
    symlink(&metadata, &f.manifest).unwrap();
    assert!(f.verify().is_err());
    let alias = f.root.join("linked-installation");
    symlink(f.app.parent().unwrap(), &alias).unwrap();
    assert!(verify(&alias.join("paintcraft"), contract(), f.target, &|| Ok(())).is_err());
}

#[test]
fn oversized_executable_is_refused_from_metadata_without_materializing_it() {
    let f = Installation::new(Target::LinuxX64, false);
    fs::OpenOptions::new()
        .write(true)
        .open(&f.helper)
        .unwrap()
        .set_len(EXECUTABLE_BYTES_MAX + 1)
        .unwrap();
    assert_eq!(f.verify().unwrap_err().code, ErrorCode::ResourceLimit);
}
#[cfg(unix)]
#[test]
fn ordinary_fifo_helper_is_rejected_without_blocking_on_open() {
    let f = Installation::new(Target::LinuxX64, false);
    fs::remove_file(&f.helper).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&f.helper)
            .status()
            .unwrap()
            .success()
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    let app = f.app.clone();
    let target = f.target;
    let worker = std::thread::spawn(move || {
        sender
            .send(verify(&app, contract(), target, &|| Ok(())))
            .unwrap()
    });
    assert!(
        receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("FIFO verification blocked")
            .is_err()
    );
    worker.join().unwrap();
}

#[test]
fn unreleased_abi_environments_are_not_relabelled_as_gnu_or_msvc() {
    for (os, arch, environment) in [
        ("linux", "x86_64", "musl"),
        ("linux", "aarch64", ""),
        ("windows", "x86_64", "gnu"),
        ("windows", "aarch64", "gnullvm"),
    ] {
        assert!(Target::from_environment(os, arch, environment).is_err());
    }
    assert_eq!(
        Target::from_environment("linux", "aarch64", "gnu")
            .unwrap()
            .triple(),
        "aarch64-unknown-linux-gnu"
    );
    assert_eq!(
        Target::from_environment("windows", "x86_64", "msvc")
            .unwrap()
            .triple(),
        "x86_64-pc-windows-msvc"
    );
}
#[test]
fn every_build_marker_occurrence_must_be_complete_and_match_the_contract() {
    let f = Installation::new(Target::LinuxX64, false);
    for suffix in [
        MARKER.to_vec(),
        [MARKER, b"0.1"].concat(),
        RECORD
            .iter()
            .map(|b| if *b == b'H' { b'Q' } else { *b })
            .collect(),
    ] {
        let mut bytes = binary(f.target);
        bytes.extend(suffix);
        fs::write(&f.helper, bytes).unwrap();
        f.refresh();
        assert!(f.verify().is_err());
    }
}

#[test]
fn qualified_variants_are_exact_bounded_and_old_schema_is_not_admitted() {
    let f = Installation::new(Target::LinuxX64, true);
    for variants in [
        Value::Array(vec![]),
        Value::Array(vec![
            Value::str("qualified-test-reader"),
            Value::str("qualified-test-reader"),
        ]),
        Value::Array(vec![
            Value::str("qualified-test-reader"),
            Value::str("foreign-reader"),
        ]),
        Value::Array(vec![Value::Int(1)]),
        Value::Array(vec![Value::str("qualified-test-reader"); 9]),
    ] {
        let Value::Object(mut fields) = f.metadata() else {
            panic!()
        };
        fields.insert("qualified_import_variants".into(), variants);
        fs::write(&f.manifest, json::canonical(&Value::Object(fields))).unwrap();
        assert!(f.verify().is_err());
    }
    let Value::Object(mut fields) = f.metadata() else {
        panic!()
    };
    fields.insert("schema_version".into(), Value::Int(1));
    fields.remove("qualified_import_variants");
    fields.insert("public_reader_enabled".into(), Value::Bool(false));
    fs::write(&f.manifest, json::canonical(&Value::Object(fields))).unwrap();
    assert!(f.verify().is_err());
    let mut expected = contract();
    expected.qualified_import_variants = &["duplicate", "duplicate"];
    assert_eq!(
        expected.validate().unwrap_err().code,
        ErrorCode::InputSchema
    );
}

const ALT_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V5_TEST\0";
const ALT_RECORD: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V5_TEST\0\x30.1.0\0PCDEC002\0HEAP268435456\0AVIF_ALLOC1\0END_CODEC_WORKER_BUILD\0";
fn alternate() -> Contract<'static> {
    Contract {
        profile: "charged-avif-test",
        qualified_import_variants: &["qualified-test-reader", "qualified-avif-reader"],
        build_marker: ALT_MARKER,
        build_record: ALT_RECORD,
        ..contract()
    }
}
fn install_alternate(value: &Installation) {
    let mut bytes = binary(value.target);
    bytes.truncate(128);
    bytes.extend(ALT_RECORD);
    fs::write(&value.helper, bytes).unwrap();
    let mut metadata = value.metadata();
    let Value::Object(map) = &mut metadata else {
        unreachable!()
    };
    map.insert("profile".into(), Value::str(alternate().profile));
    map.insert(
        "qualified_import_variants".into(),
        Value::Array(
            alternate()
                .qualified_import_variants
                .iter()
                .map(|v| Value::str(*v))
                .collect(),
        ),
    );
    fs::write(&value.manifest, json::canonical(&metadata)).unwrap();
}
#[test]
fn finite_profiles_select_before_integrity_checks_and_do_not_fallback() {
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::WinArm,
        Target::WinX64,
        Target::LinuxArm,
        Target::LinuxX64,
    ] {
        let value = Installation::new(target, false);
        verify_known(
            &value.app,
            contract(),
            Some(alternate()),
            target,
            &|| Ok(()),
        )
        .unwrap();
        assert!(
            verify(&value.app, alternate(), target, &|| Ok(())).is_err(),
            "base helper cannot satisfy AVIF-only contract"
        );
        install_alternate(&value);
        verify_known(
            &value.app,
            contract(),
            Some(alternate()),
            target,
            &|| Ok(()),
        )
        .unwrap();
        verify(&value.app, alternate(), target, &|| Ok(())).unwrap();
        assert!(verify(&value.app, contract(), target, &|| Ok(())).is_err());
        fs::OpenOptions::new()
            .append(true)
            .open(&value.helper)
            .unwrap()
            .write_all(b"changed")
            .unwrap();
        let error = verify_known(
            &value.app,
            contract(),
            Some(alternate()),
            target,
            &|| Ok(()),
        )
        .unwrap_err();
        assert!(error.message.contains("SHA-256"), "{}", error.message);
    }
}
#[test]
fn alternate_profiles_reject_bad_variant_marker_cancellation_and_contracts() {
    let value = Installation::new(Target::current().unwrap(), false);
    install_alternate(&value);
    let mut metadata = json::parse(&fs::read(&value.manifest).unwrap()).unwrap();
    let Value::Object(map) = &mut metadata else {
        unreachable!()
    };
    map.insert(
        "qualified_import_variants".into(),
        Value::Array(vec![Value::str("qualified-test-reader")]),
    );
    fs::write(&value.manifest, json::canonical(&metadata)).unwrap();
    assert!(
        verify_known(
            &value.app,
            contract(),
            Some(alternate()),
            value.target,
            &|| Ok(())
        )
        .is_err()
    );
    install_alternate(&value);
    let mut bytes = binary(value.target);
    bytes.truncate(128);
    bytes.extend(RECORD);
    fs::write(&value.helper, bytes).unwrap();
    let mut metadata = json::parse(&fs::read(&value.manifest).unwrap()).unwrap();
    let Value::Object(map) = &mut metadata else {
        unreachable!()
    };
    map.insert(
        "sha256".into(),
        Value::str(hex(&Sha256::digest(fs::read(&value.helper).unwrap()))),
    );
    fs::write(&value.manifest, json::canonical(&metadata)).unwrap();
    assert!(
        verify_known(
            &value.app,
            contract(),
            Some(alternate()),
            value.target,
            &|| Ok(())
        )
        .is_err(),
        "AVIF manifest cannot bless a base build record"
    );
    install_alternate(&value);
    let error = verify_known(
        &value.app,
        contract(),
        Some(alternate()),
        value.target,
        &|| Err(Error::new(ErrorCode::OperationCancelled, "cancel")),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let incompatible = Contract {
        requested_heap_ceiling_bytes: 128 << 20,
        ..alternate()
    };
    assert_eq!(
        verify_known(
            &value.app,
            contract(),
            Some(incompatible),
            value.target,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}

mod three;

mod four;

#[path = "tests/encoder.rs"]
mod encoder;

#[path = "tests/mixed.rs"]
mod mixed;
