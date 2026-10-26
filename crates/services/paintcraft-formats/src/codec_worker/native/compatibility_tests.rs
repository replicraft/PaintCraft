//! Copied immutable V7 installation through the ordinary legacy native Job APIs.
use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    resources::{ResourceLease, SharedBudget},
};
use paintcraft_runtime::process;
use std::{cell::Cell, fs, process::Command};
const INNER: &str = "codec_worker::native::compatibility_tests::installed_v7_legacy_fixture";
fn fixture(group: &str, name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(group)
            .join(name),
    )
    .unwrap()
}
fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("target"),
    }
}
fn manifest(helper: &Path) -> Value {
    Value::object([
        ("schema_version", Value::Int(2)),
        ("embedded_build_record_verified", Value::Bool(true)),
        ("application_version", Value::str(env!("CARGO_PKG_VERSION"))),
        ("target", Value::str(target())),
        (
            "executable",
            Value::str(helper.file_name().unwrap().to_str().unwrap()),
        ),
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
    ])
}
#[test]
#[ignore = "requires PAINTCRAFT_V7_HELPER pointing to immutable qualified V7 helper"]
fn copied_v7_installation_keeps_legacy_native_jobs_compatible() {
    let helper = PathBuf::from(std::env::var_os("PAINTCRAFT_V7_HELPER").expect("V7 helper"));
    let current = std::env::current_exe().unwrap();
    let root = std::env::temp_dir().join(format!("paintcraft-v7-legacy-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let application = root.join(current.file_name().unwrap());
    fs::copy(current, &application).unwrap();
    let installed = root.join(if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    });
    fs::copy(helper, &installed).unwrap();
    fs::write(
        root.join(codec_helper::MANIFEST_NAME),
        json::canonical(&manifest(&installed)),
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
fn launch(launch: Launch<'_>, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    let output = process::run_parent_lease(
        launch.executable,
        launch.arguments,
        launch.request.segments(),
        process::Limits {
            input_bytes: INPUT_BYTES_MAX + REQUEST_HEADER,
            output_bytes: OUTPUT_BYTES_MAX,
            deadline: launch.remaining,
            resource_exit_code: Some(RESOURCE_EXIT),
        },
        check,
    )?;
    assert!(output.status.success());
    Ok(output.bytes)
}
fn pixels(doc: &paintcraft_document::Document, expected: &[u8]) {
    let layer = doc.layers_in_order().next().unwrap();
    assert_eq!(
        expected.len(),
        doc.canvas.0 as usize * doc.canvas.1 as usize * 4
    );
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
#[ignore = "invoked only by copied V7 installation"]
fn installed_v7_legacy_fixture() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let path = root.join("source");
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let resident = SharedBudget::new(8 << 20);
    let transient = SharedBudget::new(384 << 20);
    let mut cases = vec![
        (
            Codec::JxlRgba8,
            fixture("jxl-fixtures", "rgba8.jxl"),
            vec![
                10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150,
                254, 160, 170, 180, 255,
            ],
            vec![],
        ),
        (
            Codec::JxrRgb8,
            fixture("jxr-fixtures", "rgb8-dpi96x144.jxr"),
            (0..3)
                .flat_map(|y| {
                    (0..4).flat_map(move |x| {
                        [
                            ((x * 47 + y * 13) % 256) as u8,
                            ((x * 47 + y * 13 + 71) % 256) as u8,
                            ((x * 47 + y * 13 + 142) % 256) as u8,
                            255,
                        ]
                    })
                })
                .collect(),
            vec!["jxr-resolution-not-retained"],
        ),
        (
            Codec::AvifSrgb8,
            fixture("avif-fixtures", "alpha.avif"),
            fixture("avif-fixtures", "alpha.libaom.rgba"),
            vec![],
        ),
        (
            Codec::AvifSdr8,
            fixture("avif-sdr-fixtures", "420-vertical-limited-odd-alpha.avif"),
            fixture(
                "avif-sdr-fixtures",
                "420-vertical-limited-odd-alpha.rational.rgba",
            ),
            vec!["avif-color-reconstruction"],
        ),
    ];
    let rgb = fixture("heif-color-fixtures", "identity444.rational-rgb");
    cases.push((
        Codec::HeifSrgb8,
        fixture("heif-color-fixtures", "identity444.heic"),
        rgb.as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        vec!["heif-item-names-not-retained"],
    ));
    for (codec, raw, expected, losses) in cases {
        fs::write(&path, &raw).unwrap();
        let current = Cell::new(owner);
        let job = || Job {
            input: path.clone(),
            identity: SourceIdentity::from_file(fs::File::open(&path).unwrap()).unwrap(),
            length: raw.len() as u64,
            codec,
            owner,
            pixels: PIXELS_MAX,
            deadline: Duration::from_secs(15),
            application_version: env!("CARGO_PKG_VERSION").into(),
            expected_sha256: Some(hex(&Sha256::digest(&raw))),
        };
        let prepare = |change: u8| {
            job().prepare_import(
                resident.clone(),
                ResourceLease::new(
                    transient
                        .reserve(reservation_bytes_for_input(raw.len() as u64).unwrap())
                        .unwrap(),
                ),
                &|| Ok(()),
                &|| Ok(current.get()),
                |context, check| {
                    let output = launch(context, check)?;
                    if change == 1 {
                        let mut stale = owner;
                        stale.epoch += 1;
                        current.set(stale);
                    } else if change == 2 {
                        fs::write(&path, b"changed after child").unwrap();
                    }
                    Ok(output)
                },
            )
        };
        let prepared = prepare(0).unwrap();
        assert_eq!(prepared.losses(), losses);
        pixels(prepared.document(), &expected);
        assert!(resident.usage().reserved > 0);
        assert!(transient.usage().reserved > 0);
        let allowed = losses.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let adopted = prepared
            .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
            .unwrap();
        assert_eq!(transient.usage().reserved, 0);
        assert!(resident.usage().reserved > 0);
        assert_eq!(fs::read(&path).unwrap(), raw);
        pixels(
            &crate::pcnd::read(&crate::pcnd::write(&adopted.document).unwrap()).unwrap(),
            &expected,
        );
        drop(adopted);
        assert_eq!(resident.usage().reserved, 0);
        assert!(prepare(1).is_err());
        assert_eq!(resident.usage().reserved, 0);
        assert_eq!(transient.usage().reserved, 0);
        current.set(owner);
        assert!(prepare(2).is_err());
        assert_eq!(resident.usage().reserved, 0);
        assert_eq!(transient.usage().reserved, 0);
        fs::write(&path, &raw).unwrap();
        println!(
            "V7 ordinary native Job {codec:?}: exact independent pixels/PCND, actual losses, source/resident/stale-owner guards"
        );
    }
    let raw = fixture("heif-fixtures", "primary.heic");
    fs::write(&path, &raw).unwrap();
    let _gray_lease = ResourceLease::new(
        transient
            .reserve(reservation_bytes_for_input(raw.len() as u64).unwrap())
            .unwrap(),
    );
    Job {
        input: path.clone(),
        identity: SourceIdentity::from_file(fs::File::open(&path).unwrap()).unwrap(),
        length: raw.len() as u64,
        codec: Codec::HeifGray8,
        owner,
        pixels: PIXELS_MAX,
        deadline: Duration::from_secs(15),
        application_version: env!("CARGO_PKG_VERSION").into(),
        expected_sha256: None,
    }
    .consume(&|| Ok(()), &|| Ok(owner), launch, |_, frame, _, _| {
        let Pixels::NativeGray8 { gray, alpha, .. } = frame.pixels else {
            panic!("native gray")
        };
        let expected: Vec<_> = (0..24)
            .flat_map(|y| (0..32).map(move |x| ((x * 7 + y * 11) % 256) as u8))
            .collect();
        assert_eq!(gray, expected);
        assert!(alpha.is_none());
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), raw);
    let helper = root.join(if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    });
    for (field, value) in [
        ("profile", Value::str("unknown-future-v8")),
        (
            "profile",
            Value::str(
                "private-native-v7-hdr-exr-sdr8-settings32-report96-16mib-1mp-parent-lease1",
            ),
        ),
        ("sha256", Value::str("0".repeat(64))),
        ("qualified_import_variants", Value::Array(vec![])),
        ("profile", Value::str(AVIF_SDR_NATIVE_PROFILE)),
    ] {
        let Value::Object(mut metadata) = manifest(&helper) else {
            unreachable!()
        };
        metadata.insert(field.into(), value);
        fs::write(
            root.join(codec_helper::MANIFEST_NAME),
            json::canonical(&Value::Object(metadata)),
        )
        .unwrap();
        for codec in [
            Codec::JxlRgba8,
            Codec::JxrRgb8,
            Codec::HeifGray8,
            Codec::HeifSrgb8,
            Codec::AvifSrgb8,
            Codec::AvifSdr8,
        ] {
            assert!(
                Job {
                    input: path.clone(),
                    identity: SourceIdentity::from_file(fs::File::open(&path).unwrap()).unwrap(),
                    length: raw.len() as u64,
                    codec,
                    owner,
                    pixels: PIXELS_MAX,
                    deadline: Duration::from_secs(15),
                    application_version: env!("CARGO_PKG_VERSION").into(),
                    expected_sha256: None
                }
                .consume(
                    &|| Ok(()),
                    &|| Ok(owner),
                    |_, _| panic!("forged contract must not launch"),
                    |_, _, _, _| Ok(())
                )
                .is_err()
            );
        }
    }
    fs::write(
        root.join(codec_helper::MANIFEST_NAME),
        json::canonical(&manifest(&helper)),
    )
    .unwrap();
    println!(
        "V7 native discovery: all six legacy codecs, unknown/forged manifests refused before launch; no retry"
    );
}
