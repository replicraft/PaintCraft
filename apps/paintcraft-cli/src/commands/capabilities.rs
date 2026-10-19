use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {}

pub fn run(_a: Options, json: bool) -> Reply {
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
                (
                    "capabilities",
                    Value::Array(vec![
                        row(
                            "pcnd.rgba8.read",
                            "paintcraft.native/zip1-document1",
                            "read",
                            "paintcraft-formats",
                        ),
                        row(
                            "pcnd.rgba8.write",
                            "paintcraft.native/zip1-document1",
                            "fresh-write",
                            "paintcraft-formats",
                        ),
                        row("batch.v1.edit", POLICY, "edit", "paintcraft-editor"),
                        row("flat.render", POLICY, "render", "paintcraft-render"),
                        row(
                            "png.rgba8.write",
                            "png/rgba8-straight",
                            "fresh-write",
                            "paintcraft-formats",
                        ),
                    ]),
                ),
            ]);
            (
                value,
                "pcnd read/write, batch v1 edit, flat render, png write (none release-qualified)"
                    .to_owned(),
            )
        }),
    )
}
