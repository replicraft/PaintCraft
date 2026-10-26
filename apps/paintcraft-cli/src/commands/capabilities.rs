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
                            if family == Family::Avif {
                                fields.insert(
                                    "required_helper_profile".into(),
                                    Value::str(
                                        if variant == codec_worker::NATIVE_AVIF_SDR_IMPORT_VARIANT {
                                            codec_worker::AVIF_SDR_NATIVE_PROFILE
                                        } else {
                                            codec_worker::AVIF_NATIVE_PROFILE
                                        },
                                    ),
                                );
                            }
                        }
                        frontend_rows.push(value);
                    }
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
