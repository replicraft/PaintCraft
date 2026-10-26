use super::*;
#[test]
fn checked_geometry_bound_covers_all_allowed_widths_and_thin_extremes() {
    let maximum = float_max_resident_bytes().unwrap();
    assert_eq!(maximum, 256 * RasterLayer::TILE_BYTES as u64);
    assert_eq!(float_resident_bytes(65536, 16).unwrap(), maximum);
    assert_eq!(float_resident_bytes(1, 65536).unwrap(), maximum);
    assert_eq!(
        float_resident_bytes(257, 1).unwrap(),
        2 * RasterLayer::TILE_BYTES as u64
    );
    for size in [(0, 1), (65537, 1), (65536, 17), (1025, 1024)] {
        assert!(float_resident_bytes(size.0, size.1).is_err());
    }
    let n = 16777216;
    assert_eq!(
        float_reservation_bytes(n).unwrap(),
        HEAP_BYTES + n * 2 + 32 + super::super::super::float::OUTPUT_BYTES_MAX as u64 + 65536
    );
    assert!(float_reservation_bytes(0).is_err());
    assert!(float_reservation_bytes(n + 1).is_err());
}
use paintcraft_core::types::json::{self, Value};
use paintcraft_runtime::process;
use std::{cell::Cell, fs, process::Command};
const INNER: &str = "codec_worker::native::float::tests::installed_v7_float_fixture";
fn fixture(name: &str) -> Vec<u8> {
    let group = if name.ends_with(".hdr") && name != "aspect-2.hdr" {
        "radiance-fixtures"
    } else if name.ends_with(".exr") {
        "exr-fixtures"
    } else {
        "float-import-fixtures"
    };
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(group)
            .join(name),
    )
    .unwrap()
}
fn settings(v: &Value) -> precision::Settings {
    use precision::*;
    let field = |name| v.get(name).and_then(Value::as_str);
    Settings {
        curve: if field("curve") == Some("reinhard") {
            Curve::ReinhardPerChannel
        } else {
            Curve::Clip
        },
        negative: if field("negative") == Some("reject") {
            Negative::Reject
        } else {
            Negative::ClipToBlack
        },
        exposure_milli_ev: v.get("exposure_milli_ev").unwrap().as_i64().unwrap() as i32,
        source_assignment: field("assignment").map(|s| {
            if s == "rec709" {
                LinearSpace::Rec709D65
            } else {
                LinearSpace::RadianceEqualEnergy
            }
        }),
        alpha_assignment: field("alpha_assignment").map(|s| {
            if s == "straight" {
                AlphaAssignment::Straight
            } else {
                AlphaAssignment::Premultiplied
            }
        }),
        alpha_output: v.get("background").and_then(Value::as_array).map_or(
            AlphaOutput::Preserve,
            |a| AlphaOutput::Flatten {
                srgb_background: std::array::from_fn(|i| a[i].as_i64().unwrap() as u8),
            },
        ),
    }
}
fn owner() -> Owner {
    Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    }
}
fn pixels(doc: &paintcraft_document::Document, expected: &[u8]) {
    let layer = doc.layers_in_order().next().unwrap();
    for (i, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get(
                    (i % doc.canvas.0 as usize) as i64,
                    (i / doc.canvas.0 as usize) as i64
                )
                .0,
            *pixel
        );
    }
}
#[test]
#[ignore = "requires PAINTCRAFT_V7_HELPER pointing to the qualified opt-in V7 binary"]
fn copied_installed_v7_float_job() {
    let helper =
        PathBuf::from(std::env::var_os("PAINTCRAFT_V7_HELPER").expect("qualified V7 helper path"));
    let current = std::env::current_exe().unwrap();
    let root = std::env::temp_dir().join(format!("paintcraft-float-installed-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let application = root.join(current.file_name().unwrap());
    fs::copy(current, &application).unwrap();
    let helper_name = if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    };
    fs::copy(&helper, root.join(helper_name)).unwrap();
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("target"),
    };
    let metadata = Value::object([
        ("schema_version", Value::Int(2)),
        ("embedded_build_record_verified", Value::Bool(true)),
        ("application_version", Value::str(env!("CARGO_PKG_VERSION"))),
        ("target", Value::str(target)),
        ("executable", Value::str(helper_name)),
        (
            "sha256",
            Value::str(hex(&Sha256::digest(fs::read(helper).unwrap()))),
        ),
        ("protocol_version", Value::Int(i64::from(PROTOCOL_VERSION))),
        ("profile", Value::str(FLOAT_NATIVE_PROFILE)),
        (
            "requested_heap_ceiling_bytes",
            Value::Int(HEAP_BYTES as i64),
        ),
        (
            "qualified_import_variants",
            Value::Array(
                QUALIFIED_FLOAT_IMPORT_VARIANTS
                    .iter()
                    .map(|v| Value::str(*v))
                    .collect(),
            ),
        ),
    ]);
    fs::write(
        root.join(codec_helper::MANIFEST_NAME),
        json::canonical(&metadata),
    )
    .unwrap();
    let output = Command::new(application)
        .args(["--ignored", "--exact", INNER, "--nocapture"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    fs::remove_dir_all(root).unwrap();
}
#[test]
#[ignore = "invoked only in the copied exact V7 installation"]
fn installed_v7_float_fixture() {
    let manifest = json::parse(&fixture("manifest.json")).unwrap();
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let path = root.join("source.float");
    let mut count = 0;
    for case in manifest.get("cases").unwrap().as_array().unwrap() {
        let field = |k| case.get(k).and_then(Value::as_str);
        let raw = fixture(field("file").unwrap());
        assert_eq!(hex(&Sha256::digest(&raw)), field("source_sha256").unwrap());
        fs::write(&path, &raw).unwrap();
        let settings = settings(case.get("settings").unwrap());
        let codec = if field("family") == Some("hdr") {
            Codec::HdrSdr8
        } else {
            Codec::ExrSdr8
        };
        let owner = owner();
        let current = Cell::new(owner);
        let cancel = Cell::new(false);
        let resident = SharedBudget::new(float_max_resident_bytes().unwrap());
        let metadata = SharedBudget::new(8 << 20);
        let transient = SharedBudget::new(float_reservation_bytes(raw.len() as u64).unwrap());
        let make = |during: u8| {
            let job = Job {
                input: path.clone(),
                identity: SourceIdentity::from_file(fs::File::open(&path).unwrap()).unwrap(),
                length: raw.len() as u64,
                codec,
                owner,
                pixels: super::super::super::float::PIXELS_MAX,
                deadline: Duration::from_secs(10),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: Some(hex(&Sha256::digest(&raw))),
            };
            job.prepare_float(
                settings,
                resident.clone(),
                metadata.clone(),
                ResourceLease::new(
                    transient
                        .reserve(float_reservation_bytes(raw.len() as u64).unwrap())
                        .unwrap(),
                ),
                &|| {
                    if cancel.get() {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                    } else {
                        Ok(())
                    }
                },
                &|| Ok(current.get()),
                |selected, plan| {
                    assert_eq!(selected, codec);
                    assert_eq!(
                        plan.binding().source_sha256(),
                        <[u8; 32]>::from(Sha256::digest(&raw))
                    );
                    assert!(
                        resident.usage().reserved
                            >= float_resident_bytes(
                                plan.binding().size().0,
                                plan.binding().size().1
                            )
                            .unwrap()
                    );
                    Ok(())
                },
                |launch, check| {
                    assert_ne!(
                        launch.request.header.sha256,
                        <[u8; 32]>::from(Sha256::digest(&raw))
                    );
                    let output = process::run_parent_lease(
                        launch.executable,
                        launch.arguments,
                        launch.request.segments(),
                        process::Limits {
                            input_bytes: super::super::super::float::INPUT_BYTES_MAX
                                + REQUEST_HEADER,
                            output_bytes: super::super::super::float::OUTPUT_BYTES_MAX,
                            deadline: launch.remaining,
                            resource_exit_code: Some(RESOURCE_EXIT),
                        },
                        check,
                    )?;
                    assert!(output.status.success());
                    match during {
                        1 => fs::write(&path, b"changed").unwrap(),
                        2 => {
                            let mut o = owner;
                            o.epoch += 1;
                            current.set(o);
                        }
                        3 => cancel.set(true),
                        _ => {}
                    }
                    Ok(output.bytes)
                },
            )
        };
        if field("expected_error").is_some() {
            assert!(make(0).is_err());
            continue;
        }
        let expected = fixture(field("expected_rgba_file").unwrap());
        let prepared = make(0).unwrap();
        assert_eq!(
            prepared.decoder_profile(),
            if codec == Codec::HdrSdr8 {
                NATIVE_HDR_SDR_IMPORT_VARIANT
            } else {
                NATIVE_EXR_SDR_IMPORT_VARIANT
            }
        );
        pixels(prepared.document(), &expected);
        let digest = prepared.float_assessment().unwrap().digest();
        let allowed: Vec<_> = prepared.losses().iter().map(|s| s.to_string()).collect();
        assert!(metadata.usage().reserved > 0);
        assert!(
            prepared
                .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
                .is_err()
        );
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(resident.usage().reserved, 0);
        assert!(
            make(0)
                .unwrap()
                .adopt_float([0; 32], &allowed, &|| Ok(()), &|| Ok(owner))
                .is_err()
        );
        assert!(
            make(0)
                .unwrap()
                .adopt_float(digest, &[], &|| Ok(()), &|| Ok(owner))
                .is_err()
        );
        let adopted = make(0)
            .unwrap()
            .adopt_float(digest, &allowed, &|| Ok(()), &|| Ok(owner))
            .unwrap();
        assert_eq!(transient.usage().reserved, 0);
        assert!(metadata.usage().reserved > 0);
        assert_eq!(adopted.sha256, <[u8; 32]>::from(Sha256::digest(&raw)));
        pixels(
            &crate::pcnd::read(&crate::pcnd::write(&adopted.document).unwrap()).unwrap(),
            &expected,
        );
        let receipt = adopted.float_assessment.as_ref().unwrap().clone();
        drop(adopted);
        assert_eq!(resident.usage().reserved, 0);
        assert!(metadata.usage().reserved > 0);
        drop(receipt);
        assert_eq!(metadata.usage().reserved, 0);
        for before_adoption in 1..=3 {
            let prepared = make(0).unwrap();
            if before_adoption == 1 {
                fs::write(&path, b"changed").unwrap();
            }
            let mut foreign = owner;
            foreign.epoch += 1;
            assert!(
                prepared
                    .adopt_float(
                        digest,
                        &allowed,
                        &|| if before_adoption == 3 {
                            Err(Error::new(
                                ErrorCode::OperationCancelled,
                                "cancel before publication",
                            ))
                        } else {
                            Ok(())
                        },
                        &|| Ok(if before_adoption == 2 { foreign } else { owner })
                    )
                    .is_err()
            );
            assert_eq!(resident.usage().reserved, 0);
            assert_eq!(metadata.usage().reserved, 0);
            assert_eq!(transient.usage().reserved, 0);
            fs::write(&path, &raw).unwrap();
        }
        for during in 1..=3 {
            assert!(make(during).is_err());
            assert_eq!(resident.usage().reserved, 0);
            assert_eq!(metadata.usage().reserved, 0);
            assert_eq!(transient.usage().reserved, 0);
            current.set(owner);
            cancel.set(false);
            fs::write(&path, &raw).unwrap();
        }
        count += 1;
    }
    assert!(count >= 10);
    println!(
        "V7 native Job: {count} independent exact pixel/PCND cases, raw/envelope hashes, digest/loss refusal, source/owner/cancellation, resident and retained metadata lifetimes passed"
    );
}
#[test]
fn stale_cancel_and_frontend_checks_precede_source_admission_and_launch() {
    let root = std::env::temp_dir().join(format!("paintcraft-float-check-{}", Uuid::new_v4()));
    fs::write(&root, b"raw").unwrap();
    let settings = settings(
        &json::parse(br#"{"curve":"clip","negative":"clip","exposure_milli_ev":0}"#).unwrap(),
    );
    let owner = owner();
    for kind in 0..3 {
        let mut job_owner = owner;
        if kind == 2 {
            job_owner.frontend = 1;
        }
        let job = Job {
            input: root.clone(),
            identity: SourceIdentity::from_file(fs::File::open(&root).unwrap()).unwrap(),
            length: 3,
            codec: Codec::HdrSdr8,
            owner: job_owner,
            pixels: 1,
            deadline: Duration::from_secs(1),
            application_version: String::new(),
            expected_sha256: None,
        };
        let prep = SharedBudget::new(1);
        let error = job
            .prepare_float(
                settings,
                SharedBudget::new(1),
                SharedBudget::new(1),
                ResourceLease::new(prep.reserve(1).unwrap()),
                &|| {
                    if kind == 0 {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                    } else {
                        Ok(())
                    }
                },
                &|| {
                    let mut foreign = owner;
                    foreign.epoch += 1;
                    Ok(foreign)
                },
                |_, _| panic!("must not admit"),
                |_, _| panic!("must not launch"),
            )
            .unwrap_err();
        assert_eq!(
            error.code,
            match kind {
                0 => ErrorCode::OperationCancelled,
                1 => ErrorCode::ConflictStaleBase,
                _ => ErrorCode::CapabilityUnsupported,
            }
        );
        assert_eq!(prep.usage().reserved, 0);
    }
    fs::remove_file(root).unwrap();
}
#[test]
fn raw_digest_metadata_residency_and_frontend_refusals_precede_discovery() {
    let path = std::env::temp_dir().join(format!("paintcraft-float-admit-{}", Uuid::new_v4()));
    let raw = fixture("literal-2x2.hdr");
    fs::write(&path, &raw).unwrap();
    let settings = settings(
        &json::parse(br#"{"curve":"clip","negative":"clip","exposure_milli_ev":0}"#).unwrap(),
    );
    let owner = owner();
    for kind in 0..4 {
        let metadata = SharedBudget::new(if kind == 1 { 1 } else { 8 << 20 });
        let resident = SharedBudget::new(if kind == 2 { 1 } else { 1 << 20 });
        let preparation = SharedBudget::new(float_reservation_bytes(raw.len() as u64).unwrap());
        let job = Job {
            input: path.clone(),
            identity: SourceIdentity::from_file(fs::File::open(&path).unwrap()).unwrap(),
            length: raw.len() as u64,
            codec: Codec::HdrSdr8,
            owner,
            pixels: 4,
            deadline: Duration::from_secs(10),
            application_version: String::new(),
            expected_sha256: Some(if kind == 0 {
                "0".repeat(64)
            } else {
                hex(&Sha256::digest(&raw))
            }),
        };
        let error = job
            .prepare_float(
                settings,
                resident.clone(),
                metadata.clone(),
                ResourceLease::new(
                    preparation
                        .reserve(float_reservation_bytes(raw.len() as u64).unwrap())
                        .unwrap(),
                ),
                &|| Ok(()),
                &|| Ok(owner),
                |_, _| {
                    assert_eq!(kind, 3);
                    Err(unsupported("frontend refuses float"))
                },
                |_, _| panic!("must not launch"),
            )
            .unwrap_err();
        assert_eq!(
            error.code,
            match kind {
                0 => ErrorCode::ConflictStaleBase,
                1 | 2 => ErrorCode::ResourceLimit,
                _ => ErrorCode::CapabilityUnsupported,
            }
        );
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(resident.usage().reserved, 0);
        assert_eq!(preparation.usage().reserved, 0);
    }
    fs::remove_file(path).unwrap();
}
