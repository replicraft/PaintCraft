//! --format never grants permission to overwrite an acquired project or alias.
use std::{fs, process::Command};
#[test]
fn actual_generic_render_refuses_same_path_hardlink_and_symlink_source_aliases() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-render-copy-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let original = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    );
    fs::write(&source, original).unwrap();
    let hard = root.join("hard.png");
    fs::hard_link(&source, &hard).unwrap();
    let paths = vec![source.clone(), hard];
    #[cfg(unix)]
    let paths = {
        let mut paths = paths;
        let link = root.join("symbolic.png");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        paths.push(link);
        paths
    };
    for path in paths {
        let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("render")
            .arg(&source)
            .args([
                "--format",
                "png",
                "--overwrite",
                "--source-metadata",
                "omit",
                "--json",
            ])
            .arg("-o")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            !result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let diagnostic = String::from_utf8_lossy(&result.stdout);
        let expected = if path == source {
            "extension_mismatch"
        } else {
            "must not replace its captured source project"
        };
        assert!(diagnostic.contains(expected), "{diagnostic}");
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    fs::remove_dir_all(root).unwrap();
}
