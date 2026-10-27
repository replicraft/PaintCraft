//! Actual final installation reuses the established legacy and float baselines.
use super::super::jxr_encode::qualified;
use super::*;
use std::{fs, process::Command};
#[test]
#[ignore = "requires explicit immutable qualified public V8 producer"]
fn copied_public_v8_preserves_legacy_and_float_native_jobs() {
    let source = PathBuf::from(
        std::env::var_os("PAINTCRAFT_JXR_ENCODE_HELPER").expect("qualified public helper"),
    );
    let root = std::env::temp_dir().join(format!("paintcraft-v8-compat-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let current = std::env::current_exe().unwrap();
    let application = root.join(current.file_name().unwrap());
    fs::copy(&current, &application).unwrap();
    let helper = root.join(source.file_name().unwrap());
    fs::copy(&source, &helper).unwrap();
    let manifest = fs::read(source.with_file_name(codec_helper::MANIFEST_NAME)).unwrap();
    let path = root.join(codec_helper::MANIFEST_NAME);
    fs::write(&path, &manifest).unwrap();
    let record = qualified::build_record(env!("CARGO_PKG_VERSION")).unwrap();
    let contract = codec_helper::EncoderContract {
        decode: codec_helper::Contract {
            application_version: env!("CARGO_PKG_VERSION"),
            protocol_version: PROTOCOL_VERSION,
            profile: qualified::NATIVE_PROFILE,
            requested_heap_ceiling_bytes: HEAP_BYTES,
            qualified_import_variants: qualified::IMPORT_VARIANTS,
            build_marker: qualified::BUILD_RECORD_MARKER,
            build_record: &record,
        },
        encode_protocol_version: super::super::jxr_encode::VERSION,
        qualified_export_variants: qualified::EXPORT_VARIANTS,
    };
    let verified =
        codec_helper::for_application_decoder(&application, contract, &|| Ok(())).unwrap();
    assert_eq!(verified.checked_executable(&|| Ok(())).unwrap(), helper);
    for (label, test) in [
        (
            "legacy",
            "codec_worker::native::compatibility_tests::installed_v7_legacy_fixture",
        ),
        (
            "float",
            "codec_worker::native::float::tests::installed_v7_float_fixture",
        ),
    ] {
        // Legacy baseline deliberately tampers with its fixture manifest. Each
        // baseline starts from the producer's actual descriptor, never a relabel.
        fs::write(&path, &manifest).unwrap();
        let output = Command::new(&application)
            .args([
                "--ignored",
                "--exact",
                test,
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .unwrap();
        fs::write(root.join(format!("{label}.log")), &output.stdout).unwrap();
        fs::write(root.join(format!("{label}.stderr")), &output.stderr).unwrap();
        eprintln!("qualified V8 {label} evidence {}", root.display());
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(&path, &manifest).unwrap();
    // Deliberate fixture rewrites invalidate the original installation pin.
    assert_eq!(
        verified.checked_executable(&|| Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    codec_helper::for_application_decoder(&application, contract, &|| Ok(()))
        .unwrap()
        .checked_executable(&|| Ok(()))
        .unwrap();
}
