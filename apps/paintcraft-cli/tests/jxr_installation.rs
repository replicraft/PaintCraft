//! Actual CLI route; no helper grant, source document or destination is supplied.
use serde_json::Value;
use std::process::Command;
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn legacy_discovery_stays_generation_zero_and_scoped_installation_is_explicit() {
    let legacy = run(&["capabilities", "--json"]);
    assert!(legacy.status.success());
    let legacy: Value = serde_json::from_slice(&legacy.stdout).unwrap();
    assert_eq!(legacy["capability_generation"], "0");
    let scoped = run(&["capabilities", "--installation-jxr", "--json"]);
    assert!(
        scoped.status.success(),
        "{}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    let scoped: Value = serde_json::from_slice(&scoped.stdout).unwrap();
    assert_eq!(scoped["schema"], "paintcraft.jxr-installation/v1");
    assert_eq!(scoped["release_qualified"], false);
    assert!(scoped["available"].is_boolean());
    if scoped["available"] == true {
        assert_eq!(scoped["helper_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(scoped["manifest_sha256"].as_str().unwrap().len(), 64);
    } else {
        assert!(scoped["helper_sha256"].is_null());
    }
    assert!(
        !run(&[
            "capabilities",
            "--installation-jxr",
            "--frontend",
            "classic-v7",
            "--json"
        ])
        .status
        .success()
    );
}
