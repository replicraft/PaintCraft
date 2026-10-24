//! Native print UI is isolated in a small AppKit helper; Rust remains safe.
use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=resources/macos/Print.swift");
    if env::var_os("CARGO_FEATURE_PRINTING").is_none()
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
    {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies output directory"));
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        other => panic!("unsupported native macOS print helper architecture: {other:?}"),
    };
    let status = Command::new("/usr/bin/xcrun")
        .args([
            "swiftc",
            "-O",
            "-target",
            &format!("{arch}-apple-macosx13.0"),
        ])
        .arg("-module-cache-path")
        .arg(out.join("swift-module-cache"))
        .args(["-framework", "AppKit", "resources/macos/Print.swift", "-o"])
        .arg(out.join("paintcraft-print"))
        .status()
        .expect("macOS printing requires Xcode command-line tools and swiftc");
    assert!(
        status.success(),
        "failed to compile native macOS print helper"
    );
}
