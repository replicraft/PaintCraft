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
            .map(|layer| {
                let mut fields = std::collections::BTreeMap::from([
                    ("id".into(), Value::str(layer.id.to_string())),
                    ("name".into(), Value::str(&layer.name)),
                    ("visible".into(), Value::Bool(layer.visible)),
                    ("opacity_u8".into(), Value::Int(i64::from(layer.opacity))),
                    (
                        "origin".into(),
                        Value::object([
                            ("x", Value::Int(layer.origin.0)),
                            ("y", Value::Int(layer.origin.1)),
                        ]),
                    ),
                    ("width".into(), Value::Int(i64::from(layer.raster.width()))),
                    (
                        "height".into(),
                        Value::Int(i64::from(layer.raster.height())),
                    ),
                    (
                        "default_pixel".into(),
                        Value::str(layer.raster.default_pixel().to_hex()),
                    ),
                ]);
                if !layer.filters.is_empty() {
                    fields.insert(
                        "filters".into(),
                        paintcraft_document::filters::inspection(&layer.filters),
                    );
                }
                Value::Object(fields)
            })
            .collect(),
    )
}

/// `paintcraft.open-assessment/v1`
fn assessment(sha: &str, m: &pcnd::Header, mode: &str, reasons: &[String]) -> Value {
    let profile = match (m.require_supported().is_ok(), m.container_version) {
        (true, 1) => "paintcraft.native/binary1-document1",
        (true, 2) => "paintcraft.native/binary2-document2-source-metadata1",
        (true, 3) => "paintcraft.native/binary3-document3-point-filters1",
        (true, 4) => "paintcraft.native/binary4-document4-filter-masks1",
        (true, 5) => {
            "paintcraft.native/binary5-document5-point-filters1-filter-masks1-spatial-filters1"
        }
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

fn catalog_assessment(
    sha: &str,
    header: &pcnd::Header,
    mode: &str,
    reasons: &[String],
    bytes: &[u8],
) -> Result<Value> {
    let mut result = assessment(sha, header, mode, reasons);
    if header.container_version == 6 {
        let requirements = pcnd::catalog_manifest(bytes)?;
        let Value::Object(ref mut fields) = result else {
            unreachable!("assessment object");
        };
        fields.insert(
            "schema".into(),
            Value::str("paintcraft.open-assessment/catalog-v1"),
        );
        // Header support describes grammar only. A known adoption profile is
        // reported only after the complete document reader accepted every pair.
        if mode == "editable" && header.require_supported().is_ok() {
            fields.insert(
                "profile".into(),
                Value::str("paintcraft.native/binary6-document6-filter-catalog1"),
            );
        }
        fields.insert(
            "required_filters".into(),
            Value::Array(
                requirements
                    .into_iter()
                    .map(|requirement| {
                        Value::object([
                            ("profile_id", Value::str(requirement.profile_id)),
                            ("codec_id", Value::str(requirement.codec_id)),
                        ])
                    })
                    .collect(),
            ),
        );
    }
    Ok(result)
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
    match super::source_metadata::read(&bytes) {
        Ok(doc) => {
            let header = pcnd::probe(&bytes).expect("a readable document has a valid header");
            let assessed = match catalog_assessment(&sha, &header, "editable", &[], &bytes) {
                Ok(value) => value,
                Err(error) => return bare(json, Err(error)),
            };
            let mut value = Value::object([
                (
                    "schema",
                    Value::str(super::filter_inspection::cli_schema(&doc)),
                ),
                ("assessment", assessed),
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
            let features = paintcraft_editor::session::masked_document_features(&doc);
            let requirements = paintcraft_editor::session::catalog_document_requirements(&doc);
            if !requirements.is_empty() {
                let Value::Object(ref mut fields) = value else {
                    unreachable!("inspection object");
                };
                fields.insert(
                    "required_filters".into(),
                    Value::Array(requirements.into_iter().map(Value::str).collect()),
                );
            }
            if !features.is_empty() {
                let Value::Object(fields) = &mut value else {
                    unreachable!("object")
                };
                fields.insert(
                    "document_features".into(),
                    Value::Array(features.iter().copied().map(Value::str).collect()),
                );
            }
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
                    let value = match catalog_assessment(
                        &sha,
                        &m,
                        "reject",
                        std::slice::from_ref(&e.message),
                        &bytes,
                    ) {
                        Ok(value) => value,
                        Err(error) => return bare(json, Err(error)),
                    };
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

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn unknown_required_profiles_and_codecs_never_claim_an_adopted_profile() {
        let oracle = paintcraft_core::types::json::parse(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/services/paintcraft-formats/tests/pcnd-catalog-fixtures/oracle.json"
        )))
        .unwrap();
        for name in [
            "unknown-profile",
            "unknown-disabled-profile",
            "unknown-codec",
            "unknown-disabled-codec",
        ] {
            let fixture = oracle
                .get("fixtures")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .find(|fixture| fixture.get("name").and_then(Value::as_str) == Some(name))
                .unwrap();
            let hex = fixture.get("hex").unwrap().as_str().unwrap().as_bytes();
            let (pairs, remainder) = hex.as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes: Vec<u8> = pairs
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let header = pcnd::probe(&bytes).unwrap();
            assert!(header.require_supported().is_ok()); // Grammar is known.
            assert_eq!(
                super::super::source_metadata::read(&bytes)
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            let value = catalog_assessment(
                "unused",
                &header,
                "reject",
                &["unsupported requirement".into()],
                &bytes,
            )
            .unwrap();
            assert_eq!(
                value.get("schema").and_then(Value::as_str),
                Some("paintcraft.open-assessment/catalog-v1")
            );
            assert_eq!(
                value.get("profile").and_then(Value::as_str),
                Some("paintcraft.native/unrecognized")
            );
            let requirements = value.get("required_filters").unwrap().as_array().unwrap();
            assert_eq!(requirements.len(), 1);
            assert!(
                requirements[0]
                    .get("profile_id")
                    .and_then(Value::as_str)
                    .is_some()
            );
        }
    }
}
