//! The CLI commands. Each returns what to print and how to exit; nothing here
//! calls `process::exit`.

use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use paintcraft_core::document::{Document, POLICY, hex, new_document};
use paintcraft_core::render::render_flat;
use paintcraft_core::session::Session;
use paintcraft_formats::{pcnd, png};
use paintcraft_types::json::Value;
use paintcraft_types::{Error, ErrorCode, Result, Rgba};
use sha2::{Digest, Sha256};

use crate::args::Args;
use crate::report::{envelope, error_value, json_line};
use crate::save::{self, Context, Outcome};

const INPUT_MAX: u64 = 1 << 30;
const OPERATIONS_MAX: u64 = 64 << 20;
const BATCH_SCHEMA: &str = include_str!("../schemas/batch.schema.json");

pub struct Reply {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit: u8,
}

/// The result of a command that writes a file.
struct Written {
    engine: Option<Value>,
    receipt: Value,
    ok: bool,
    summary: String,
}

fn storage(what: &str, e: &std::io::Error) -> Error {
    Error::new(ErrorCode::StorageFailed, format!("{what}: {}", e.kind()))
}

/// Acquire one immutable snapshot of the input and its digest.
fn read_bounded(path: &str, limit: u64, what: &str) -> Result<(Vec<u8>, String)> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|e| storage(&format!("cannot read {what}"), &e))?;
    if bytes.len() as u64 > limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            format!("{what} exceeds the {limit}-byte limit"),
        ));
    }
    let sha = hex(&Sha256::digest(&bytes));
    Ok((bytes, sha))
}

/// `--expect-input-sha256` guards the exact acquired bytes before any parsing.
fn guard(args: &Args, actual: &str) -> Result<()> {
    let Some(expected) = args.value("expect-input-sha256") else {
        return Ok(());
    };
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "--expect-input-sha256 must be 64 lowercase hex digits",
        ));
    }
    if expected != actual {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "input bytes differ from --expect-input-sha256",
        ));
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

fn finish(json: bool, result: Result<Written>) -> Reply {
    match result {
        Err(e) => {
            let exit = e.code.exit_code();
            let stdout = if json {
                json_line(&envelope(
                    Some(error_value(&e)),
                    None,
                    "engine-rejected",
                    exit,
                ))
            } else {
                Vec::new()
            };
            Reply {
                stdout,
                stderr: format!("error: {e}\n"),
                exit,
            }
        }
        Ok(w) => {
            let exit = if w.ok {
                0
            } else {
                ErrorCode::StorageFailed.exit_code()
            };
            let outcome = if w.ok { "success" } else { "file-incomplete" };
            let stdout = if json {
                json_line(&envelope(w.engine, Some(w.receipt), outcome, exit))
            } else {
                format!("{}\n", w.summary).into_bytes()
            };
            Reply {
                stdout,
                stderr: if w.ok {
                    String::new()
                } else {
                    format!("error: {}\n", w.summary)
                },
                exit,
            }
        }
    }
}

fn bare(json: bool, result: Result<(Value, String)>) -> Reply {
    match result {
        Ok((value, text)) => Reply {
            stdout: if json {
                json_line(&value)
            } else {
                format!("{text}\n").into_bytes()
            },
            stderr: String::new(),
            exit: 0,
        },
        Err(e) => Reply {
            stdout: if json {
                json_line(&error_value(&e))
            } else {
                Vec::new()
            },
            stderr: format!("error: {e}\n"),
            exit: e.code.exit_code(),
        },
    }
}

/// Write `bytes` (or record why they could not be produced) and build the receipt.
fn write_output(
    dest: &Path,
    overwrite: bool,
    bytes: Result<Vec<u8>>,
    ctx: &Context<'_>,
) -> (Value, bool, String) {
    let outcome: Outcome = match bytes {
        Ok(b) => save::save(dest, &b, overwrite),
        Err(e) => save::not_attempted(&e),
    };
    let summary = if outcome.is_success() {
        format!("wrote {}", dest.display())
    } else {
        format!(
            "{} was not saved safely: {}",
            dest.display(),
            outcome.reasons.join("; ")
        )
    };
    (outcome.receipt(ctx), outcome.is_success(), summary)
}

pub fn new(raw: &[String]) -> Reply {
    let json = raw.iter().any(|a| a == "--json");
    finish(
        json,
        (|| {
            let a = Args::parse(
                raw,
                &["width", "height", "policy", "background", "seed", "output"],
                &["json", "overwrite", "deterministic-ids"],
            )?;
            a.exactly(0, "no positional arguments")?;
            let (w, h) = (a.number::<u32>("width")?, a.number::<u32>("height")?);
            let (w, h) = (
                w.ok_or_else(|| Error::new(ErrorCode::InputSchema, "--width is required"))?,
                h.ok_or_else(|| Error::new(ErrorCode::InputSchema, "--height is required"))?,
            );
            let background = a
                .value("background")
                .map(|c| {
                    Rgba::parse(c).ok_or_else(|| {
                        Error::new(ErrorCode::InputSchema, "--background must be #RRGGBBAA")
                    })
                })
                .transpose()?;
            let dest = Path::new(a.required("output")?);
            save::preflight(dest, a.flag("overwrite"))?;
            // Identifiers are always the deterministic allocator today; --deterministic-ids is accepted for forward compatibility.
            let doc = new_document(
                w,
                h,
                background,
                a.number::<u64>("seed")?.unwrap_or(0),
                a.value("policy").unwrap_or(POLICY),
            )?;
            let ctx = Context {
                operation: "save-as",
                source_state: doc.state_id,
                source_sha256: None,
                losses_acknowledged: vec![],
                destination: dest,
            };
            let (receipt, ok, summary) =
                write_output(dest, a.flag("overwrite"), pcnd::write(&doc), &ctx);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}

pub fn apply(raw: &[String]) -> Reply {
    let json = raw.iter().any(|a| a == "--json");
    finish(
        json,
        (|| {
            let a = Args::parse(
                raw,
                &["operations", "output", "expect-input-sha256"],
                &["json", "overwrite"],
            )?;
            let input = &a.exactly(1, "one input file")?[0];
            let dest = Path::new(a.required("output")?);
            save::preflight(dest, a.flag("overwrite"))?;
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            guard(&a, &sha)?;
            let (ops, _) =
                read_bounded(a.required("operations")?, OPERATIONS_MAX, "operations file")?;
            let mut session = Session::new(pcnd::read(&bytes)?);
            let receipt = session.apply_json(&ops)?;
            let ctx = Context {
                operation: if same_file(Path::new(input), dest) {
                    "save"
                } else {
                    "save-as"
                },
                source_state: session.document().state_id,
                source_sha256: Some(sha),
                losses_acknowledged: vec![],
                destination: dest,
            };
            let (file, ok, summary) = write_output(
                dest,
                a.flag("overwrite"),
                pcnd::write(session.document()),
                &ctx,
            );
            Ok(Written {
                engine: Some(receipt.to_value()),
                receipt: file,
                ok,
                summary,
            })
        })(),
    )
}

pub fn render(raw: &[String]) -> Reply {
    let json = raw.iter().any(|a| a == "--json");
    finish(
        json,
        (|| {
            let a = Args::parse(
                raw,
                &["output", "expect-input-sha256", "on-loss", "allow-loss"],
                &["json", "overwrite"],
            )?;
            let input = &a.exactly(1, "one input file")?[0];
            let dest = Path::new(a.required("output")?);
            if a.value("on-loss").is_some_and(|v| v != "error") {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "--on-loss supports only `error`; acknowledge a loss with --allow-loss",
                ));
            }
            let allowed: Vec<String> = a
                .value("allow-loss")
                .map(|v| v.split(',').map(str::to_owned).collect())
                .unwrap_or_default();
            if let Some(bad) = allowed.iter().find(|c| *c != "flatten") {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    format!("unknown loss code {bad:?}; known: flatten"),
                ));
            }
            save::preflight(dest, a.flag("overwrite"))?;
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            guard(&a, &sha)?;
            let doc = pcnd::read(&bytes)?;
            // A flat export of several layers discards their structure. Hidden-RGB loss is not tracked yet.
            let mut losses = Vec::new();
            if doc.roots.len() > 1 {
                if !allowed.iter().any(|c| c == "flatten") {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "PNG export would flatten layers; pass --allow-loss flatten to accept",
                    ));
                }
                losses.push("flatten".to_owned());
            }
            let png =
                render_flat(&doc).and_then(|px| png::encode_rgba8(doc.canvas.0, doc.canvas.1, &px));
            let ctx = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: Some(sha),
                losses_acknowledged: losses,
                destination: dest,
            };
            let (receipt, ok, summary) = write_output(dest, a.flag("overwrite"), png, &ctx);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
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
fn assessment(sha: &str, m: &pcnd::Manifest, mode: &str, reasons: &[String]) -> Value {
    let known = m.container_version == 1 && m.document_schema == "paintcraft.document/v1";
    Value::object([
        ("schema", Value::str("paintcraft.open-assessment/v1")),
        ("assessment_only", Value::Bool(true)),
        ("source_sha256", Value::str(sha)),
        (
            "profile",
            Value::str(if known {
                "paintcraft.native/zip1-document1"
            } else {
                "paintcraft.native/unrecognized"
            }),
        ),
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

pub fn inspect(raw: &[String]) -> Reply {
    let json = raw.iter().any(|a| a == "--json");
    let parsed = Args::parse(raw, &["expect-input-sha256"], &["json"]).and_then(|a| {
        let input = a.exactly(1, "one input file")?[0].clone();
        let (bytes, sha) = read_bounded(&input, INPUT_MAX, "input")?;
        guard(&a, &sha)?;
        Ok((bytes, sha))
    });
    let (bytes, sha) = match parsed {
        Ok(v) => v,
        Err(e) => return bare(json, Err(e)),
    };
    match pcnd::read(&bytes) {
        Ok(doc) => {
            let manifest = pcnd::probe(&bytes).expect("a readable document has a valid manifest");
            let value = Value::object([
                ("schema", Value::str("paintcraft.inspect/v1")),
                ("assessment", assessment(&sha, &manifest, "editable", &[])),
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

pub fn schema(raw: &[String]) -> Reply {
    // The schema is the output; it is always JSON, with or without --json.
    bare(
        true,
        (|| {
            let a = Args::parse(raw, &[], &["json"])?;
            if a.exactly(1, "`schema batch`")?[0] != "batch" {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "only `schema batch` exists",
                ));
            }
            Ok((
                paintcraft_types::json::parse(BATCH_SCHEMA.as_bytes())
                    .expect("embedded schema is valid JSON"),
                String::new(),
            ))
        })(),
    )
}

pub fn capabilities(raw: &[String]) -> Reply {
    let json = raw.iter().any(|a| a == "--json");
    bare(
        json,
        Args::parse(raw, &[], &["json"]).and_then(|a| a.exactly(0, "no positional arguments").map(|_| ())).map(|()| {
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
                ("app_build", Value::str(format!("paintcraft-cli {}", env!("CARGO_PKG_VERSION")))),
                ("environment", Value::str("host-unqualified")),
                ("capability_generation", Value::str("0")),
                ("synthetic", Value::Bool(false)),
                (
                    "capabilities",
                    Value::Array(vec![
                        row("pcnd.rgba8.read", "paintcraft.native/zip1-document1", "read", "paintcraft-formats"),
                        row("pcnd.rgba8.write", "paintcraft.native/zip1-document1", "fresh-write", "paintcraft-formats"),
                        row("batch.v1.edit", POLICY, "edit", "paintcraft-core"),
                        row("flat.render", POLICY, "render", "paintcraft-core"),
                        row("png.rgba8.write", "png/rgba8-straight", "fresh-write", "paintcraft-formats"),
                    ]),
                ),
            ]);
            (value, "pcnd read/write, batch v1 edit, flat render, png write (none release-qualified)".to_owned())
        }),
    )
}
