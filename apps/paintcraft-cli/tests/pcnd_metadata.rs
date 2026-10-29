//! Native source attachments are editable supported projects, not future versions.
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

#[test]
fn actual_inspect_assesses_independent_v1_and_v2_projects_without_changing_the_source() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures");
    for (name, version, profile) in [
        ("legacy-v1.pcnd", 1, "paintcraft.native/binary1-document1"),
        (
            "rgb-profile.pcnd",
            2,
            "paintcraft.native/binary2-document2-source-metadata1",
        ),
        (
            "resolution-only.pcnd",
            2,
            "paintcraft.native/binary2-document2-source-metadata1",
        ),
        (
            "orientation-only.pcnd",
            2,
            "paintcraft.native/binary2-document2-source-metadata1",
        ),
    ] {
        let path = root.join(name);
        let before = fs::read(&path).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("inspect")
            .arg(&path)
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["schema"], "paintcraft.inspect/v1");
        assert_eq!(report["assessment"]["container_version"], version);
        assert_eq!(report["assessment"]["mode"], "editable");
        assert_eq!(report["assessment"]["profile"], profile);
        assert_eq!(
            report["assessment"]["document_schema"],
            format!("paintcraft.document/v{version}")
        );
        assert_eq!(
            report["document"]["document_id"],
            "33333333-3333-4333-8333-333333333333"
        );
        assert_eq!(
            report["ticket"]["state_id"],
            "11111111-1111-4111-8111-111111111111"
        );
        let features = report["assessment"]["required_features"]
            .as_array()
            .unwrap();
        assert_eq!(
            features.iter().any(|f| f == "source-metadata.v1"),
            version == 2
        );
        assert_eq!(fs::read(path).unwrap(), before);
    }
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    let rows = report["capabilities"].as_array().unwrap();
    for capability in ["pcnd.source-metadata.read", "pcnd.source-metadata.write"] {
        let row = rows
            .iter()
            .find(|row| row["capability"] == capability)
            .unwrap();
        assert_eq!(
            row["profile"],
            "paintcraft.native/binary2-document2-source-metadata1"
        );
    }
}
