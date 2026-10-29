use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
}

fn layer_info(doc: &Document) -> Value {
    Value::Array(
        doc.layers_in_order()
            .map(|l| {
                Value::object([
                    ("id", Value::str(l.id.to_string())),
                    ("name", Value::str(&l.name)),
                    ("visible", Value::Bool(l.visible)),
                    ("opacity_u8", Value::Int(i64::from(l.opacity))),
                    (
                        "origin",
                        Value::object([
                            ("x", Value::Int(l.origin.0)),
                            ("y", Value::Int(l.origin.1)),
                        ]),
                    ),
                    ("width", Value::Int(i64::from(l.raster.width()))),
                    ("height", Value::Int(i64::from(l.raster.height()))),
                    (
                        "default_pixel",
                        Value::str(l.raster.default_pixel().to_hex()),
                    ),
                ])
            })
            .collect(),
    )
}

/// `paintcraft.open-assessment/v1`
fn assessment(sha: &str, m: &pcnd::Header, mode: &str, reasons: &[String]) -> Value {
    let profile = match (m.require_supported().is_ok(), m.container_version) {
        (true, 1) => "paintcraft.native/binary1-document1",
        (true, 2) => "paintcraft.native/binary2-document2-source-metadata1",
        _ => "paintcraft.native/unrecognized",
    };
    Value::object([
        ("schema", Value::str("paintcraft.open-assessment/v1")),
        ("assessment_only", Value::Bool(true)),
        ("source_sha256", Value::str(sha)),
        ("profile", Value::str(profile)),
        ("container_version", Value::Int(m.container_version)),
        ("document_schema", Value::str(&m.document_schema)),
        (
            "required_features",
            Value::Array(m.required_features.iter().map(Value::str).collect()),
        ),
        ("capability_generation", Value::str("0")),
        ("mode", Value::str(mode)),
        ("migration_id", Value::Null),
        (
            "reasons",
            Value::Array(reasons.iter().map(Value::str).collect()),
        ),
    ])
}

pub fn run(a: Options, json: bool) -> Reply {
    let parsed = (|| {
        let (bytes, sha) = read_bounded(&a.input, INPUT_MAX, "input")?;
        guard(a.expect_input_sha256.as_deref(), &sha)?;
        Ok((bytes, sha))
    })();
    let (bytes, sha) = match parsed {
        Ok(v) => v,
        Err(e) => return bare(json, Err(e)),
    };
    match pcnd::read(&bytes) {
        Ok(doc) => {
            let header = pcnd::probe(&bytes).expect("a readable document has a valid header");
            let value = Value::object([
                ("schema", Value::str("paintcraft.inspect/v1")),
                ("assessment", assessment(&sha, &header, "editable", &[])),
                (
                    "document",
                    Value::object([
                        ("document_id", Value::str(doc.document_id.to_string())),
                        ("state_id", Value::str(doc.state_id.to_string())),
                        (
                            "canvas",
                            Value::object([
                                ("width", Value::Int(i64::from(doc.canvas.0))),
                                ("height", Value::Int(i64::from(doc.canvas.1))),
                            ]),
                        ),
                        ("policy", Value::str(POLICY)),
                        ("layers", layer_info(&doc)),
                    ]),
                ),
                (
                    "ticket",
                    Value::object([
                        ("state_id", Value::str(doc.state_id.to_string())),
                        ("publication_epoch", Value::str("0")),
                    ]),
                ),
                ("input_sha256", Value::str(&sha)),
            ]);
            let text = format!(
                "{}x{} canvas, {} layer(s), state {}, sha256 {sha}",
                doc.canvas.0,
                doc.canvas.1,
                doc.roots.len(),
                doc.state_id
            );
            Reply {
                stdout: if json {
                    json_line(&value)
                } else {
                    format!("{text}\n").into_bytes()
                },
                stderr: String::new(),
                exit: 0,
            }
        }
        Err(e)
            if matches!(
                e.code,
                ErrorCode::CapabilityUnsupported | ErrorCode::PolicyUnknown
            ) =>
        {
            match pcnd::probe(&bytes) {
                Ok(m) => {
                    let value = assessment(&sha, &m, "reject", std::slice::from_ref(&e.message));
                    Reply {
                        stdout: if json { json_line(&value) } else { Vec::new() },
                        stderr: format!("rejected: {e}\n"),
                        exit: e.code.exit_code(),
                    }
                }
                Err(_) => bare(json, Err(e)),
            }
        }
        Err(e) => bare(json, Err(e)),
    }
}
