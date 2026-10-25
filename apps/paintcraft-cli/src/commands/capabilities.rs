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
                for family in registry::enabled(frontend, Operation::Open, Platform::Native) {
                    frontend_rows.push(row(
                        &format!("{}.rgba8.read", family.id()),
                        &format!("frontend/{}", frontend.id()),
                        "read",
                        registry::capability(family, Operation::Open, Platform::Native)
                            .backend
                            .unwrap_or("unavailable"),
                    ));
                }
            }
            let mut rows = vec![
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
                "pcnd read/write, batch v1 edit, flat render, image layer insertion/placement, png write (none release-qualified)"
                    .to_owned(),
            )
        }),
    )
}
