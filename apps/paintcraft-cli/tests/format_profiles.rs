use paintcraft_core::types::json;
use std::{fs, path::PathBuf, process::Command};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn frontend_flags_share_admission_and_keep_the_existing_json_envelope() {
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-profile-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("source.png");
    let png = paintcraft_formats::image::encode(
        paintcraft_formats::image::ImageFormat::Png,
        1,
        1,
        &[9, 8, 7, 0],
        90,
    )
    .unwrap();
    fs::write(&source, &png).unwrap();
    let path = |name: &str| -> PathBuf { dir.join(name) };
    for profile in ["classic-v7", "classic-v10", "classic-v11", "studio"] {
        let output = path(&format!("{profile}.pcnd"));
        let reply = run(&[
            "--json",
            "import",
            source.to_str().unwrap(),
            "--frontend",
            profile,
            "-o",
            output.to_str().unwrap(),
        ]);
        assert!(
            reply.status.success(),
            "{} {}",
            String::from_utf8_lossy(&reply.stdout),
            String::from_utf8_lossy(&reply.stderr)
        );
        let rendered = path(&format!("{profile}.png"));
        assert!(
            run(&[
                "--json",
                "render",
                output.to_str().unwrap(),
                "--frontend",
                profile,
                "--allow-loss",
                "hidden-rgb",
                "-o",
                rendered.to_str().unwrap()
            ])
            .status
            .success()
        );
        let unsupported = path("foreign.svg");
        fs::write(&unsupported, &png).unwrap();
        let denied = path(&format!("{profile}-denied.pcnd"));
        let reply = run(&[
            "--json",
            "import",
            unsupported.to_str().unwrap(),
            "--frontend",
            profile,
            "-o",
            denied.to_str().unwrap(),
        ]);
        assert_eq!(reply.status.code(), Some(3));
        assert!(!denied.exists());
        let value = json::parse(&reply.stdout).unwrap();
        assert!(value.get("schema").is_some());
        let reply = run(&["--json", "capabilities", "--frontend", profile]);
        assert!(reply.status.success());
        let value = json::parse(&reply.stdout).unwrap();
        let text = String::from_utf8(reply.stdout).unwrap();
        assert!(text.contains("frontend/"));
        assert!(text.contains("paintcraft.build-capabilities/v1"));
        assert!(value.get("schema").is_some());
    }
    let wrong = path("wrong.jpg");
    fs::write(&wrong, &png).unwrap();
    let output = path("wrong.pcnd");
    assert_eq!(
        run(&[
            "--json",
            "import",
            wrong.to_str().unwrap(),
            "-o",
            output.to_str().unwrap()
        ])
        .status
        .code(),
        Some(2)
    );
    assert!(!output.exists());
    fs::remove_dir_all(dir).unwrap();
}
