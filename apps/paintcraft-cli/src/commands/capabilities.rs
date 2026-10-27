use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    /// Include qualified discovery rows for this frontend without changing the envelope.
    #[arg(long, value_enum)]
    frontend: Option<super::format_profile::Profile>,
}

pub fn run(a: Options, json: bool) -> Reply {
    bare(
        json,
        Ok({
            let row = |capability: &str, profile: &str, operation: &str, backend: &str| {
                Value::object([
                    ("capability", Value::str(capability)),
                    ("profile", Value::str(profile)),
                    ("operation", Value::str(operation)),
                    ("backend_id", Value::str(backend)),
                    ("available", Value::Bool(true)),
                    ("enabled", Value::Bool(true)),
                    ("release_qualified", Value::Bool(false)),
                    ("implementation", Value::str("implemented")),
                    ("applicability", Value::str("applicable")),
                    ("evidence", Value::str("unverified")),
                    ("conformance", Value::str("unknown")),
                    ("disposition", Value::str("planned")),
                    ("qualification_kind", Value::str("native-core")),
                    ("result_status", Value::str("not-run")),
                    ("evidence_ids", Value::Array(vec![])),
                    ("artifact_sha256", Value::Array(vec![])),
                ])
            };
            let mut frontend_rows = Vec::new();
            if let Some(frontend) = a.frontend {
                use paintcraft_formats::registry::{self, Frontend, Operation, Platform};
                let frontend: Frontend = frontend.into();
                for (operation, suffix, wire) in [
                    (Operation::Open, "read", "read"),
                    (Operation::ImportConvert, "import", "import"),
                    (Operation::ExportFlat, "write", "fresh-write"),
                ] {
                    for family in registry::enabled(frontend, operation, Platform::Native) {
                        frontend_rows.push(row(
                            &format!("{}.rgba8.{suffix}", family.id()),
                            &format!("frontend/{}", frontend.id()),
                            wire,
                            registry::capability(family, operation, Platform::Native)
                                .backend
                                .unwrap_or("unavailable"),
                        ));
                    }
                }
            }
            // A qualified variant does not turn on the generic family reader.
            if a.frontend
                .is_none_or(|profile| matches!(profile, super::format_profile::Profile::Studio))
            {
                use paintcraft_formats::{
                    codec_worker,
                    registry::{self, Family, Frontend, Operation, Platform, Request},
                };
                for (family, variant, id, backend) in [
                    (
                        Family::Jxl,
                        codec_worker::NATIVE_JXL_IMPORT_VARIANT,
                        "jxl.rgba8.import",
                        "jxl-rs/native-worker",
                    ),
                    (
                        Family::Jxr,
                        codec_worker::NATIVE_JXR_IMPORT_VARIANT,
                        "jxr.rgb8.import",
                        "jpegxr-pure-rs/native-worker",
                    ),
                    (
                        Family::Jxr,
                        codec_worker::jxr_encode::qualified::WRITER_IMPORT_VARIANT,
                        "jxr.writer-rgb8.import",
                        "jpegxr-pure-rs/native-worker",
                    ),
                    (
                        Family::Heif,
                        codec_worker::NATIVE_HEIF_IMPORT_VARIANT,
                        "heif.srgb8.import",
                        "heic-decoder/native-worker",
                    ),
                    (
                        Family::Avif,
                        codec_worker::NATIVE_AVIF_IMPORT_VARIANT,
                        "avif.srgb8.import",
                        "dav1d/native-worker",
                    ),
                    (
                        Family::Avif,
                        codec_worker::NATIVE_AVIF_SDR_IMPORT_VARIANT,
                        "avif.sdr-ycbcr8.import",
                        "dav1d/native-worker",
                    ),
                    (
                        Family::Hdr,
                        codec_worker::NATIVE_HDR_SDR_IMPORT_VARIANT,
                        "hdr.sdr8.import",
                        "image-hdr/native-worker",
                    ),
                    (
                        Family::Exr,
                        codec_worker::NATIVE_EXR_SDR_IMPORT_VARIANT,
                        "exr.sdr8.import",
                        "exr/native-worker",
                    ),
                ] {
                    let mut request = Request::new(
                        Frontend::Studio,
                        Operation::ImportConvert,
                        family,
                        Platform::Native,
                    );
                    request.variant = Some(variant);
                    if registry::admission(request).available() {
                        let mut value = row(id, "frontend/studio", "import", backend);
                        if let Value::Object(fields) = &mut value {
                            fields.insert("variant".into(), Value::str(variant));
                            fields.insert("platform".into(), Value::str("native"));
                            fields.insert(
                                "installation_preflight_required".into(),
                                Value::Bool(true),
                            );
                            fields
                                .insert("runtime_installation_verified".into(), Value::Bool(false));
                            let profiles = native_import_profiles(variant);
                            fields.insert(
                                "accepted_helper_profiles".into(),
                                Value::Array(profiles.iter().copied().map(Value::str).collect()),
                            );
                            if matches!(family, Family::Hdr | Family::Exr) {
                                // Preserve the historical explanatory field. Exact
                                // accepted alternatives, rather than version ordering,
                                // govern installation compatibility.
                                fields.insert(
                                    "required_helper_profile".into(),
                                    Value::str(codec_worker::FLOAT_NATIVE_PROFILE),
                                );
                                fields.insert(
                                    "required_helper_profile_is_minimum".into(),
                                    Value::Bool(false),
                                );
                            } else if family == Family::Avif {
                                fields.insert(
                                    "required_helper_profile".into(),
                                    Value::str(profiles[0]),
                                );
                                fields.insert(
                                    "required_helper_profile_is_minimum".into(),
                                    Value::Bool(true),
                                );
                            } else if variant
                                == codec_worker::jxr_encode::qualified::WRITER_IMPORT_VARIANT
                            {
                                fields.insert(
                                    "required_helper_profile".into(),
                                    Value::str(codec_worker::jxr_encode::qualified::NATIVE_PROFILE),
                                );
                                fields.insert(
                                    "required_helper_profile_is_minimum".into(),
                                    Value::Bool(false),
                                );
                            }
                        }
                        frontend_rows.push(value);
                    }
                }
            }
            if a.frontend
                .is_none_or(|profile| matches!(profile, super::format_profile::Profile::Studio))
            {
                use paintcraft_formats::{
                    codec_worker::jxr_encode::qualified,
                    registry::{self, Family, Frontend, Operation, Platform, Request},
                };
                let mut request = Request::new(
                    Frontend::Studio,
                    Operation::ExportFlat,
                    Family::Jxr,
                    Platform::Native,
                );
                request.variant = Some(qualified::EXPORT_VARIANT);
                if registry::admission(request).available() {
                    let mut value = row(
                        "jxr.rgb8.lossless.write",
                        "frontend/studio",
                        "fresh-write",
                        "jpegxr-pure-rs/native-worker",
                    );
                    if let Value::Object(fields) = &mut value {
                        fields.insert("variant".into(), Value::str(qualified::EXPORT_VARIANT));
                        fields.insert("platform".into(), Value::str("native"));
                        fields.insert("installation_preflight_required".into(), Value::Bool(true));
                        fields.insert("runtime_installation_verified".into(), Value::Bool(false));
                        fields.insert(
                            "required_helper_profile".into(),
                            Value::str(qualified::NATIVE_PROFILE),
                        );
                        fields.insert(
                            "required_helper_profile_is_minimum".into(),
                            Value::Bool(false),
                        );
                        fields.insert(
                            "accepted_helper_profiles".into(),
                            Value::Array(vec![Value::str(qualified::NATIVE_PROFILE)]),
                        );
                    }
                    frontend_rows.push(value);
                }
            }
            if a.frontend
                .is_none_or(|profile| matches!(profile, super::format_profile::Profile::Studio))
            {
                use paintcraft_formats::{
                    ora,
                    registry::{self, Family, Frontend, Operation, Platform, Request},
                };
                let mut request = Request::new(
                    Frontend::Studio,
                    Operation::ImportConvert,
                    Family::Ora,
                    Platform::Native,
                );
                request.variant = Some(ora::NEUTRAL_GROUP_IMPORT_VARIANT);
                if registry::admission(request).available() {
                    let mut value = row(
                        "ora.neutral-auto.rgba8.import",
                        "frontend/studio",
                        "import",
                        "quick-xml/png/converted-copy",
                    );
                    if let Value::Object(fields) = &mut value {
                        fields.insert(
                            "variant".into(),
                            Value::str(ora::NEUTRAL_GROUP_IMPORT_VARIANT),
                        );
                        fields.insert("platform".into(), Value::str("native"));
                        fields.insert("captured_regular_source_required".into(), Value::Bool(true));
                        fields.insert(
                            "legacy_stream_profile".into(),
                            Value::str("flat-normal-png"),
                        );
                    }
                    frontend_rows.push(value);
                }
            }
            if a.frontend
                .is_none_or(|profile| matches!(profile, super::format_profile::Profile::Studio))
            {
                use paintcraft_formats::{
                    dds,
                    registry::{self, Family, Frontend, Operation, Platform, Request},
                };
                let mut request = Request::new(
                    Frontend::Studio,
                    Operation::ExportFlat,
                    Family::Dds,
                    Platform::Native,
                );
                request.extension = Some("dds");
                request.variant = Some(dds::NATIVE_TEXTURE_EXPORT_VARIANT);
                if registry::admission(request).available() {
                    let mut value = row(
                        "dds.texture2d.write",
                        "frontend/studio",
                        "fresh-write",
                        "dds/rgba8-bc123-texture",
                    );
                    if let Value::Object(fields) = &mut value {
                        fields.insert(
                            "variant".into(),
                            Value::str(dds::NATIVE_TEXTURE_EXPORT_VARIANT),
                        );
                        fields.insert("platform".into(), Value::str("native"));
                        fields.insert("preflight_required".into(), Value::Bool(true));
                        fields.insert("installation_preflight_required".into(), Value::Bool(false));
                        fields.insert("external_helper_required".into(), Value::Bool(false));
                        fields.insert("runtime_installation_verified".into(), Value::Bool(false));
                        fields.insert("may_be_lossy".into(), Value::Bool(true));
                        fields.insert("loss_policy".into(), Value::str("actual composite losses, BC compression, Box hidden RGB normalization, linear8 quantization require consent"));
                        fields.insert(
                            "compression_policy".into(),
                            Value::str(dds::UNIFORM_LOSSY_POLICY),
                        );
                        fields.insert(
                            "encodings".into(),
                            Value::Array(["rgba8", "bc1", "bc2", "bc3"].map(Value::str).to_vec()),
                        );
                        fields.insert(
                            "transfers".into(),
                            Value::Array(["srgb", "linear8"].map(Value::str).to_vec()),
                        );
                        fields.insert(
                            "mips".into(),
                            Value::Array(["none", "full"].map(Value::str).to_vec()),
                        );
                        fields.insert(
                            "filters".into(),
                            Value::Array(["nearest", "box-stored"].map(Value::str).to_vec()),
                        );
                        fields.insert("levels_min".into(), Value::Int(1));
                        fields.insert("levels_max".into(), Value::Int(17));
                        fields.insert("levels_geometry_bounded".into(), Value::Bool(true));
                        fields.insert("mips_levels_mutually_exclusive".into(), Value::Bool(true));
                        fields.insert("alpha_constraints".into(), Value::str("bc1 requires opaque composite samples; rgba8/bc2/bc3 preserve straight alpha"));
                        fields.insert("filter_policy".into(), Value::str("nearest samples base centers; box-stored uses alpha-weighted area samples in stored transfer space, directly from the base"));
                        fields.insert(
                            "efforts".into(),
                            Value::Array(["normal", "high"].map(Value::str).to_vec()),
                        );
                        fields.insert("pixel_limit".into(), Value::Int(dds::PIXELS_MAX as i64));
                        fields.insert(
                            "encoded_bytes_limit".into(),
                            Value::Int(dds::BYTES_MAX as i64),
                        );
                    }
                    frontend_rows.push(value);
                }
            }
            let mut rows = vec![
                row(
                    "csh.shapes.read",
                    "paintcraft.csh/exact-i32-ordered-records-v1",
                    "import-asset",
                    "ag-psd-csh-exact-i32",
                ),
                row(
                    "csh.shapes.write",
                    "paintcraft.csh/exact-i32-ordered-records-v1",
                    "export-asset",
                    "ag-psd-csh-exact-i32",
                ),
                row(
                    "ase.palette.read",
                    "paintcraft.ase/finite-f32-flat-groups-v1",
                    "import-asset",
                    "ag-psd-ase-finite-f32",
                ),
                row(
                    "ase.palette.write",
                    "paintcraft.ase/finite-f32-flat-groups-v1",
                    "export-asset",
                    "ag-psd-ase-finite-f32",
                ),
                row(
                    "ico.entry-set.write",
                    "paintcraft.icons/explicit-rgba8-entry-set-v1",
                    "fresh-write",
                    "ico",
                ),
                row(
                    "cur.entry-set.write",
                    "paintcraft.icons/explicit-rgba8-entry-set-v1",
                    "fresh-write",
                    "ico",
                ),
                row(
                    "mcp.stdio.paint",
                    "paintcraft.mcp/ephemeral-v1",
                    "serve",
                    "rmcp",
                ),
                row(
                    "pcnd.rgba8.read",
                    "paintcraft.native/binary1-document1",
                    "read",
                    "paintcraft-formats",
                ),
                row(
                    "pcnd.rgba8.write",
                    "paintcraft.native/binary1-document1",
                    "fresh-write",
                    "paintcraft-formats",
                ),
                row("batch.v1.edit", POLICY, "edit", "paintcraft-editor"),
                row("flat.render", POLICY, "render", "paintcraft-render"),
                row(
                    "image.rgba8.paste-layer",
                    POLICY,
                    "paste-image",
                    "paintcraft-editor",
                ),
                row(
                    "image.rgba8.paste-onto",
                    POLICY,
                    "paste-image",
                    "paintcraft-editor",
                ),
                row(
                    "png.rgba8.write",
                    "png/rgba8-straight",
                    "fresh-write",
                    "paintcraft-formats",
                ),
            ];
            rows.extend(frontend_rows);
            let value = Value::object([
                ("schema", Value::str("paintcraft.build-capabilities/v1")),
                ("kind", Value::str("runtime")),
                (
                    "app_build",
                    Value::str(format!("paintcraft-cli {}", env!("CARGO_PKG_VERSION"))),
                ),
                ("environment", Value::str("host-unqualified")),
                ("capability_generation", Value::str("0")),
                ("synthetic", Value::Bool(false)),
                ("capabilities", Value::Array(rows)),
            ]);
            (
                value,
                "pcnd read/write, batch v1 edit, flat render, image layer insertion/placement, png write, MCP stdio painting (none release-qualified)"
                    .to_owned(),
            )
        }),
    )
}

/// Mirrors the codec-specific exact contracts in native::installed_helper.
/// These are finite identities, never a minimum-version acceptance rule.
fn native_import_profiles(variant: &str) -> &'static [&'static str] {
    use paintcraft_formats::codec_worker::{self as wire, jxr_encode::qualified};
    match variant {
        wire::NATIVE_HDR_SDR_IMPORT_VARIANT | wire::NATIVE_EXR_SDR_IMPORT_VARIANT => {
            &[wire::FLOAT_NATIVE_PROFILE, qualified::NATIVE_PROFILE]
        }
        wire::NATIVE_AVIF_SDR_IMPORT_VARIANT => &[
            wire::AVIF_SDR_NATIVE_PROFILE,
            wire::FLOAT_NATIVE_PROFILE,
            qualified::NATIVE_PROFILE,
        ],
        wire::NATIVE_AVIF_IMPORT_VARIANT => &[
            wire::AVIF_NATIVE_PROFILE,
            wire::AVIF_SDR_NATIVE_PROFILE,
            wire::FLOAT_NATIVE_PROFILE,
            qualified::NATIVE_PROFILE,
        ],
        qualified::WRITER_IMPORT_VARIANT => &[qualified::NATIVE_PROFILE],
        wire::NATIVE_JXL_IMPORT_VARIANT
        | wire::NATIVE_JXR_IMPORT_VARIANT
        | wire::NATIVE_HEIF_IMPORT_VARIANT => &[
            wire::NATIVE_PROFILE,
            wire::AVIF_NATIVE_PROFILE,
            wire::AVIF_SDR_NATIVE_PROFILE,
            wire::FLOAT_NATIVE_PROFILE,
            qualified::NATIVE_PROFILE,
        ],
        _ => &[],
    }
}
