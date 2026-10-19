//! Replays every conformance vector against the engine: receipts, undo/redo,
//! dirty state, selection, document snapshots and render digests. The canonical
//! PCND digest is verified in `paintcraft-formats`.

use std::fs;
use std::path::PathBuf;

use paintcraft_core::commands::Batch;
use paintcraft_core::document::{Document, hex, new_document};
use paintcraft_core::execute::Selection;
use paintcraft_core::render::render_flat;
use paintcraft_core::session::Session;
use paintcraft_types::json::{self, Value};
use paintcraft_types::{Error, Rgba};
use sha2::{Digest, Sha256};

const HEX_LIMIT: u64 = 64;

fn vectors() -> Vec<(String, Value)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                json::parse(&fs::read(&p).unwrap()).unwrap(),
            )
        })
        .collect()
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string {key}"))
}

fn error_view(e: &Error) -> Value {
    let mut pairs = vec![
        ("schema", Value::str("paintcraft.error/v1")),
        ("committed", Value::Bool(false)),
        ("code", Value::str(e.code.as_str())),
    ];
    if let Some(i) = e.operation_index {
        pairs.push(("operation_index", Value::Int(i as i64)));
    }
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn selection_json(sel: Selection) -> Value {
    match sel {
        Selection::Inactive => Value::object([("state", Value::str("inactive"))]),
        Selection::Empty => Value::object([("state", Value::str("active-empty"))]),
        Selection::Rect(r) => Value::object([
            ("state", Value::str("active")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(r.x0)),
                    ("y", Value::Int(r.y0)),
                    ("width", Value::Int(r.width())),
                    ("height", Value::Int(r.height())),
                ]),
            ),
        ]),
    }
}

fn snapshot(doc: &Document) -> Value {
    let layers = doc
        .layers_in_order()
        .map(|l| {
            let mut samples = Vec::new();
            l.raster.for_each_row(|r| samples.extend_from_slice(r));
            let mut rec = vec![
                ("id", Value::str(l.id.to_string())),
                ("name", Value::str(&l.name)),
                ("visible", Value::Bool(l.visible)),
                ("opacity_u8", Value::Int(i64::from(l.opacity))),
                (
                    "origin",
                    Value::object([("x", Value::Int(l.origin.0)), ("y", Value::Int(l.origin.1))]),
                ),
                ("width", Value::Int(i64::from(l.raster.width()))),
                ("height", Value::Int(i64::from(l.raster.height()))),
                (
                    "default_pixel",
                    Value::str(l.raster.default_pixel().to_hex()),
                ),
                ("samples_sha256", Value::str(hex(&Sha256::digest(&samples)))),
            ];
            if u64::from(l.raster.width()) * u64::from(l.raster.height()) <= HEX_LIMIT {
                rec.push(("samples_rgba_hex", Value::str(hex(&samples))));
            }
            Value::Object(rec.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
        })
        .collect();
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
        (
            "roots",
            Value::Array(
                doc.roots
                    .iter()
                    .map(|r| Value::str(r.to_string()))
                    .collect(),
            ),
        ),
        ("layers", Value::Array(layers)),
    ])
}

#[test]
fn every_vector_replays() {
    let all = vectors();
    assert!(all.len() >= 21, "vectors missing");
    for (file, vector) in all {
        let init = vector.get("initial").and_then(|i| i.get("new")).unwrap();
        let background = init
            .get("background")
            .and_then(Value::as_str)
            .map(|c| Rgba::parse(c).unwrap());
        let seed = init.get("seed").and_then(Value::as_i64).unwrap() as u64;
        let width = init.get("width").and_then(Value::as_i64).unwrap() as u32;
        let height = init.get("height").and_then(Value::as_i64).unwrap() as u32;
        let mut session =
            Session::new(new_document(width, height, background, seed, s(init, "policy")).unwrap());
        for (n, step) in vector
            .get("steps")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .enumerate()
        {
            let at = format!("{file} step {n}");
            let want = step.get("expect");
            match s(step, "action") {
                "apply" => {
                    let got = Batch::from_value(step.get("batch").unwrap())
                        .and_then(|b| session.apply(&b))
                        .map_or_else(|e| error_view(&e), |r| r.to_value());
                    assert_eq!(&got, want.unwrap(), "{at}");
                }
                action @ ("undo" | "redo") => {
                    let nav = if action == "undo" {
                        session.undo()
                    } else {
                        session.redo()
                    };
                    let got = nav.map_or(Value::object([("available", Value::Bool(false))]), |n| {
                        Value::object([
                            ("state_id", Value::str(n.state_id.to_string())),
                            (
                                "publication_epoch",
                                Value::str(n.publication_epoch.to_string()),
                            ),
                            (
                                "history_id",
                                n.history_id
                                    .map_or(Value::Null, |h| Value::str(h.to_string())),
                            ),
                        ])
                    });
                    assert_eq!(&got, want.unwrap(), "{at}");
                }
                "mark-saved" => session.mark_saved(),
                "check" => {
                    let want = want.unwrap();
                    assert_eq!(
                        want.get("dirty").and_then(Value::as_bool),
                        Some(session.is_dirty()),
                        "{at} dirty"
                    );
                    assert_eq!(
                        &selection_json(session.selection()),
                        want.get("selection").unwrap(),
                        "{at} selection"
                    );
                    assert_eq!(
                        &snapshot(session.document()),
                        want.get("document").unwrap(),
                        "{at} document"
                    );
                    let image = render_flat(session.document()).unwrap();
                    assert_eq!(
                        s(want, "render_sha256"),
                        hex(&Sha256::digest(&image)),
                        "{at} render"
                    );
                    if let Some(h) = want.get("render_rgba_hex") {
                        assert_eq!(h.as_str().unwrap(), hex(&image), "{at} render hex");
                    }
                    let w = u64::from(session.document().canvas.0);
                    for spot in want
                        .get("render_pixels")
                        .and_then(Value::as_array)
                        .unwrap_or(&[])
                    {
                        let (x, y) = (
                            spot.get("x").unwrap().as_i64().unwrap() as u64,
                            spot.get("y").unwrap().as_i64().unwrap() as u64,
                        );
                        let off = ((y * w + x) * 4) as usize;
                        assert_eq!(
                            s(spot, "rgba"),
                            hex(&image[off..off + 4]),
                            "{at} pixel {x},{y}"
                        );
                    }
                }
                other => panic!("{at}: unknown action {other}"),
            }
        }
    }
}
