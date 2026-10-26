use jpegxr::decode_file;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_jxr_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("fixture directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            collect_jxr_paths(&path, paths);
        } else if path.extension().is_some_and(|ext| ext == "jxr") {
            paths.push(path);
        }
    }
}

fn decode_one(path: &Path) -> usize {
    decode_file(path).expect("decode fixture").pixels().len()
}

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<usize>().ok())
        .unwrap_or(1);
    let root = std::env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("data")
        });
    if !root.is_dir() {
        eprintln!(
            "fixture directory not found: {}; pass it as the second argument",
            root.display()
        );
        return;
    }
    let mut paths = Vec::new();
    collect_jxr_paths(&root, &mut paths);
    paths.sort();

    let mut bytes = 0usize;
    for _ in 0..iterations {
        for path in &paths {
            bytes += decode_one(path);
        }
    }

    eprintln!(
        "decoder=translated iterations={} files={} decoded_bytes={}",
        iterations,
        paths.len(),
        bytes
    );
}
