//! End-to-end tests of the real binary, including the M1 definition-of-done
//! scenario: create, edit across tile boundaries, inspect, render PNG.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use paintcraft_core::types::json::{self, Value};

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        json::parse(self.stdout.as_bytes())
            .unwrap_or_else(|e| panic!("stdout is not one JSON object: {e}\n{}", self.stdout))
    }
}

fn run(args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap();
    Run {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("paintcraft-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text<'a>(v: &'a Value, path: &[&str]) -> &'a str {
    path.iter()
        .fold(v, |v, k| v.get(k).unwrap_or_else(|| panic!("missing {k}")))
        .as_str()
        .unwrap()
}

fn vector(name: &str) -> Value {
    json::parse(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../conformance")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

fn m1_batch() -> String {
    let steps = vector("M1-001.json");
    let batch = steps.get("steps").unwrap().as_array().unwrap()[0]
        .get("batch")
        .unwrap()
        .clone();
    String::from_utf8(json::canonical(&batch)).unwrap()
}

fn decode_png(path: &str) -> (u32, u32, Vec<u8>) {
    let mut r = png::Decoder::new(std::io::Cursor::new(fs::read(path).unwrap()))
        .read_info()
        .unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    (info.width, info.height, buf[..info.buffer_size()].to_vec())
}

#[path = "cli/contours.rs"]
mod contours;
#[path = "cli/crop.rs"]
mod crop;
#[path = "cli/editing.rs"]
mod editing;
#[path = "cli/formats_and_receipts.rs"]
mod formats_and_receipts;
#[path = "cli/layers.rs"]
mod layers;
#[path = "cli/parallel.rs"]
mod parallel;

#[path = "cli/orthogonal.rs"]
mod orthogonal;

#[path = "cli/selection_transform.rs"]
mod selection_transform;

#[path = "cli/brush_media.rs"]
mod brush_media;

#[path = "cli/selection_masks.rs"]
mod selection_masks;

#[path = "cli/selection_move.rs"]
mod selection_move;

#[path = "cli/clipboard_cut.rs"]
mod clipboard_cut;
