//! Replays every conformance vector and checks the canonical PCND digest at each
//! `check` step, plus write → read → write stability. Receipts, snapshots and
//! render digests are verified in `paintcraft-core`.

use std::fs;
use std::path::PathBuf;

use paintcraft_core::commands::Batch;
use paintcraft_core::document::{hex, new_document};
use paintcraft_core::session::Session;
use paintcraft_formats::pcnd;
use paintcraft_types::Rgba;
use paintcraft_types::json::{self, Value};
use sha2::{Digest, Sha256};

#[test]
fn canonical_pcnd_digests_match_and_round_trip() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 21);
    let mut checked = 0;
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let vector = json::parse(&fs::read(&path).unwrap()).unwrap();
        let init = vector.get("initial").and_then(|i| i.get("new")).unwrap();
        let background = init
            .get("background")
            .and_then(Value::as_str)
            .map(|c| Rgba::parse(c).unwrap());
        let mut session = Session::new(
            new_document(
                init.get("width").and_then(Value::as_i64).unwrap() as u32,
                init.get("height").and_then(Value::as_i64).unwrap() as u32,
                background,
                init.get("seed").and_then(Value::as_i64).unwrap() as u64,
                init.get("policy").and_then(Value::as_str).unwrap(),
            )
            .unwrap(),
        );
        for (n, step) in vector
            .get("steps")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .enumerate()
        {
            match step.get("action").and_then(Value::as_str).unwrap() {
                "apply" => {
                    let _ = Batch::from_value(step.get("batch").unwrap())
                        .and_then(|b| session.apply(&b));
                }
                "undo" => {
                    session.undo();
                }
                "redo" => {
                    session.redo();
                }
                "mark-saved" => session.mark_saved(),
                "check" => {
                    let bytes = pcnd::write(session.document()).unwrap();
                    let want = step
                        .get("expect")
                        .and_then(|e| e.get("pcnd_sha256"))
                        .and_then(Value::as_str)
                        .unwrap();
                    assert_eq!(hex(&Sha256::digest(&bytes)), want, "{file} step {n}");
                    let back = pcnd::read(&bytes).unwrap();
                    assert!(
                        back.same_semantics(session.document())
                            && back.state_id == session.document().state_id,
                        "{file} step {n} round trip"
                    );
                    assert_eq!(
                        pcnd::write(&back).unwrap(),
                        bytes,
                        "{file} step {n} rewrite"
                    );
                    checked += 1;
                }
                other => panic!("unknown action {other}"),
            }
        }
    }
    assert!(checked >= 40, "only {checked} PCND checks ran");
}
