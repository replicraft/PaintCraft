use super::*;
use paintcraft_core::types::json::{self, Value};
use std::fs;
fn binary() -> Vec<u8> {
    let mut bytes = vec![0; 128];
    if cfg!(target_os = "macos") {
        bytes[..4].copy_from_slice(&0xfeedfacfu32.to_le_bytes());
        bytes[4..8].copy_from_slice(
            &(if cfg!(target_arch = "aarch64") {
                0x0100000cu32
            } else {
                0x01000007u32
            })
            .to_le_bytes(),
        );
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
    } else if cfg!(target_os = "windows") {
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(
            &(if cfg!(target_arch = "aarch64") {
                0xaa64u16
            } else {
                0x8664u16
            })
            .to_le_bytes(),
        );
        bytes[86..88].copy_from_slice(&2u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    } else {
        bytes[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
        bytes[16..18].copy_from_slice(&3u16.to_le_bytes());
        bytes[18..20].copy_from_slice(
            &(if cfg!(target_arch = "aarch64") {
                183u16
            } else {
                62u16
            })
            .to_le_bytes(),
        );
    }
    bytes
}
#[test]
fn only_exact_v9_or_v10_installs_authorize_integer_requests() {
    let root = std::env::temp_dir().join(format!("paintcraft-jxl-contract-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let app = root.join("paintcraft");
    fs::write(&app, binary()).unwrap();
    let helper = root.join(if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    });
    let version = env!("CARGO_PKG_VERSION");
    for (schema, profile, variants, record) in [
        (
            2,
            NATIVE_PROFILE,
            QUALIFIED_NATIVE_IMPORT_VARIANTS,
            native_build_record(version).unwrap(),
        ),
        (
            2,
            AVIF_NATIVE_PROFILE,
            QUALIFIED_AVIF_IMPORT_VARIANTS,
            native_avif_build_record(version).unwrap(),
        ),
        (
            2,
            AVIF_SDR_NATIVE_PROFILE,
            QUALIFIED_AVIF_SDR_IMPORT_VARIANTS,
            native_avif_sdr_build_record(version).unwrap(),
        ),
        (
            2,
            base::NATIVE_PROFILE,
            base::IMPORT_VARIANTS,
            base::build_record(version).unwrap(),
        ),
        (
            3,
            writer::NATIVE_PROFILE,
            writer::IMPORT_VARIANTS,
            writer::build_record(version).unwrap(),
        ),
        (
            2,
            FLOAT_NATIVE_PROFILE,
            QUALIFIED_FLOAT_IMPORT_VARIANTS,
            native_float_build_record(version).unwrap(),
        ),
        (
            3,
            crate::codec_worker::jxr_encode::qualified::NATIVE_PROFILE,
            crate::codec_worker::jxr_encode::qualified::IMPORT_VARIANTS,
            crate::codec_worker::jxr_encode::qualified::build_record(version).unwrap(),
        ),
    ] {
        let mut bytes = binary();
        bytes.extend(record);
        fs::write(&helper, &bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut manifest = Value::object([
            ("schema_version", Value::Int(schema)),
            ("embedded_build_record_verified", Value::Bool(true)),
            ("application_version", Value::str(version)),
            (
                "target",
                Value::str(match (std::env::consts::OS, std::env::consts::ARCH) {
                    ("macos", "aarch64") => "aarch64-apple-darwin",
                    ("macos", "x86_64") => "x86_64-apple-darwin",
                    ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                    ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
                    ("windows", "aarch64") => "aarch64-pc-windows-msvc",
                    ("windows", "x86_64") => "x86_64-pc-windows-msvc",
                    _ => panic!("supported release host required"),
                }),
            ),
            (
                "executable",
                Value::str(helper.file_name().unwrap().to_str().unwrap()),
            ),
            ("sha256", Value::str(hex(&Sha256::digest(&bytes)))),
            ("protocol_version", Value::Int(2)),
            ("profile", Value::str(profile)),
            (
                "requested_heap_ceiling_bytes",
                Value::Int(HEAP_BYTES as i64),
            ),
            (
                "qualified_import_variants",
                Value::Array(variants.iter().map(|v| Value::str(*v)).collect()),
            ),
        ]);
        if schema == 3 {
            let Value::Object(fields) = &mut manifest else {
                unreachable!()
            };
            fields.insert("encode_protocol_version".into(), Value::Int(1));
            fields.insert(
                "qualified_export_variants".into(),
                Value::Array(
                    crate::codec_worker::jxr_encode::qualified::EXPORT_VARIANTS
                        .iter()
                        .map(|v| Value::str(*v))
                        .collect(),
                ),
            );
        }
        fs::write(
            root.join("paintcraft-codec-worker.json"),
            json::canonical(&manifest),
        )
        .unwrap();
        let new_profile = profile == base::NATIVE_PROFILE || profile == writer::NATIVE_PROFILE;
        let compatible =
            super::super::installed_helper_for_application(&app, Codec::JxlRgba8, version, &|| {
                Ok(())
            })
            .unwrap();
        assert_eq!(
            super::super::jxl_codec_for_helper(&compatible, &|| Ok(())).unwrap(),
            if new_profile {
                Codec::JxlIntegerRgba8
            } else {
                Codec::JxlRgba8
            }
        );
        assert_eq!(
            compatible.supports_import_variant(
                crate::codec_worker::jxl_integer::contracts::IMPORT_VARIANT
            ),
            new_profile
        );
        for codec in [
            Codec::JxrRgb8,
            Codec::HeifGray8,
            Codec::HeifSrgb8,
            Codec::AvifSrgb8,
            Codec::AvifSdr8,
            Codec::HdrSdr8,
            Codec::ExrSdr8,
        ] {
            let allowed = match codec {
                Codec::AvifSrgb8 => profile != NATIVE_PROFILE,
                Codec::AvifSdr8 => ![NATIVE_PROFILE, AVIF_NATIVE_PROFILE].contains(&profile),
                Codec::HdrSdr8 | Codec::ExrSdr8 => {
                    ![NATIVE_PROFILE, AVIF_NATIVE_PROFILE, AVIF_SDR_NATIVE_PROFILE]
                        .contains(&profile)
                }
                _ => true,
            };
            assert_eq!(
                super::super::installed_helper_for_application(&app, codec, version, &|| Ok(()))
                    .is_ok(),
                allowed,
                "{profile}/{codec:?}"
            );
        }
        let writer_profile = profile == writer::NATIVE_PROFILE
            || profile == crate::codec_worker::jxr_encode::qualified::NATIVE_PROFILE;
        let encoder = super::super::writer_contracts::with_contracts(version, |contracts| {
            codec_helper::for_application_encoder_set(&app, contracts, &|| Ok(()))
        });
        let writer_decoder = super::super::writer_contracts::with_contracts(version, |contracts| {
            codec_helper::for_application_decoder_compatible_set(&app, &[], contracts, &|| Ok(()))
        });
        assert_eq!(
            encoder.is_ok(),
            writer_profile,
            "writer encode role: {profile}"
        );
        assert_eq!(
            writer_decoder.is_ok(),
            writer_profile,
            "writer decode role: {profile}"
        );
        if let Ok(encoder) = &encoder {
            assert_eq!(encoder.checked_executable(&|| Ok(())).unwrap(), helper);
        }
        if let Ok(decoder) = &writer_decoder {
            assert_eq!(decoder.checked_executable(&|| Ok(())).unwrap(), helper);
        }
        let result = for_application(&app, version, &|| Ok(()));
        if new_profile {
            let verified = result.unwrap();
            assert_eq!(verified.checked_executable(&|| Ok(())).unwrap(), helper);
            assert!(verified.supports_import_variant(
                crate::codec_worker::jxl_integer::contracts::IMPORT_VARIANT
            ));
        } else {
            assert!(result.is_err());
        }
        fs::write(&helper, b"changed").unwrap();
        if let Ok(encoder) = &encoder {
            assert!(encoder.checked_executable(&|| Ok(())).is_err());
        }
        if let Ok(decoder) = &writer_decoder {
            assert!(decoder.checked_executable(&|| Ok(())).is_err());
        }
        assert!(super::super::jxl_codec_for_helper(&compatible, &|| Ok(())).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}
