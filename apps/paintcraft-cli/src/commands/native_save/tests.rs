use super::*;
use clap::Parser;
use std::fs;
const DDS: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/dx10-rgba.dds"
);
fn run(args: Vec<String>) -> Reply {
    let cli = crate::Cli::try_parse_from(args).unwrap();
    match cli.command {
        crate::Command::Import(options) => super::super::import::run(options, true),
        crate::Command::PasteImage(options) => super::super::paste_image::run(options, true),
        crate::Command::Flatten(options) => super::super::flatten::run(options, true),
        crate::Command::Brush(options) => super::super::brushes::run(options, true),
        _ => panic!("test only invokes the real import/paste commands"),
    }
}
#[test]
fn actual_import_and_paste_final_checkpoint_preserve_inputs_and_report_staged_refusal() {
    for paste in [false, true] {
        for alias in [false, true] {
            let root = std::env::temp_dir().join(format!(
                "paintcraft-native-checkpoint-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&root).unwrap();
            let source = root.join("source.dds");
            let moved = root.join("moved.dds");
            let output = root.join("output.pcnd");
            let project = root.join("input.pcnd");
            fs::write(&source, DDS).unwrap();
            fs::write(&output, DDS).unwrap();
            let document = new_document(7, 5, None, 0, POLICY).unwrap();
            let project_bytes = pcnd::write(&document).unwrap();
            fs::write(&project, &project_bytes).unwrap();
            let mut args = vec![
                "paintcraft-cli".into(),
                if paste { "paste-image" } else { "import" }.into(),
            ];
            if paste {
                args.push(project.to_str().unwrap().into());
            }
            args.extend([
                source.to_str().unwrap().into(),
                "-o".into(),
                output.to_str().unwrap().into(),
                "--overwrite".into(),
                "--frontend".into(),
                "studio".into(),
                "--allow-loss".into(),
                "dds-other-mips-not-imported".into(),
            ]);
            let (mutated_source, mutated_output, renamed) =
                (source.clone(), output.clone(), moved.clone());
            inject_at_checkpoint(
                move || {
                    if alias {
                        fs::rename(&mutated_source, &renamed).unwrap();
                        fs::remove_file(&mutated_output).unwrap();
                        fs::hard_link(&renamed, &mutated_output).unwrap();
                    } else {
                        let mut changed = DDS.to_vec();
                        *changed.last_mut().unwrap() ^= 1;
                        fs::write(&mutated_source, changed).unwrap();
                    }
                },
                || {
                    let reply = run(args.clone());
                    assert_ne!(reply.exit, 0);
                    let value: serde_json::Value = serde_json::from_slice(&reply.stdout).unwrap();
                    let file = &value["file"];
                    assert_eq!(file["replacement"], "not-replaced", "{value}");
                    assert!(file["staged_sha256"].as_str().is_some(), "{value}");
                    assert!(file["acknowledged_state_id"].is_null(), "{value}");
                    assert_eq!(file["durability"], "not-confirmed", "{value}");
                    let reasons = file["reasons"].to_string();
                    assert!(
                        reasons.contains(if alias {
                            "captured"
                        } else {
                            "Acquired source changed"
                        }),
                        "{value}"
                    );
                },
            );
            assert_eq!(fs::read(&project).unwrap(), project_bytes);
            if alias {
                assert_eq!(fs::read(&moved).unwrap(), DDS);
                assert_eq!(fs::read(&output).unwrap(), DDS);
                fs::remove_file(&output).unwrap();
                fs::rename(&moved, &source).unwrap();
            } else {
                assert_eq!(fs::read(&output).unwrap(), DDS);
                fs::write(&source, DDS).unwrap();
            }
            // Unchanged sources still publish through the same real command path.
            assert_eq!(run(args).exit, 0);
            assert_eq!(
                pcnd::read(&fs::read(&output).unwrap()).unwrap().canvas,
                (7, 5)
            );
            assert_eq!(
                fs::read_dir(&root).unwrap().count(),
                3,
                "no staging files retained on known refusal"
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn actual_project_and_brush_copies_reject_late_same_content_source_aliases() {
    const ABR: &[u8] = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/abr-fixtures/native-presets.abr"
    );
    for brush in [false, true] {
        let root = std::env::temp_dir().join(format!(
            "paintcraft-resource-checkpoint-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let project = root.join("project.pcnd");
        let resource = root.join("source.abr");
        let points = root.join("points.json");
        let output = root.join("output.pcnd");
        let moved = root.join("renamed-source");
        let original = pcnd::write(&new_document(8, 8, None, 0, POLICY).unwrap()).unwrap();
        fs::write(&project, &original).unwrap();
        fs::write(&resource, ABR).unwrap();
        fs::write(&points, b"[[3,3]]").unwrap();
        let target = if brush { &resource } else { &project };
        let original_target = fs::read(target).unwrap();
        // Same bytes defeat a content-only destination fingerprint; identity is essential.
        fs::write(&output, &original_target).unwrap();
        let mut args: Vec<String> = if brush {
            vec![
                "paintcraft-cli".into(),
                "brush".into(),
                "paint".into(),
                project.to_str().unwrap().into(),
                "--brush".into(),
                resource.to_str().unwrap().into(),
                "--preset".into(),
                "0".into(),
                "--policy".into(),
                "native-grid-v1".into(),
                "--points".into(),
                points.to_str().unwrap().into(),
                "--color".into(),
                "#123456FF".into(),
            ]
        } else {
            vec![
                "paintcraft-cli".into(),
                "flatten".into(),
                project.to_str().unwrap().into(),
            ]
        };
        args.extend([
            "-o".into(),
            output.to_str().unwrap().into(),
            "--overwrite".into(),
        ]);
        let (input, renamed, dest) = (target.clone(), moved.clone(), output.clone());
        inject_at_checkpoint(
            move || {
                fs::rename(input, &renamed).unwrap();
                fs::remove_file(&dest).unwrap();
                fs::hard_link(renamed, dest).unwrap();
            },
            || {
                let reply = run(args);
                assert_ne!(reply.exit, 0);
                let value: serde_json::Value = serde_json::from_slice(&reply.stdout).unwrap();
                let file = &value["file"];
                assert_eq!(file["replacement"], "not-replaced", "{value}");
                assert!(file["staged_sha256"].as_str().is_some(), "{value}");
                assert!(file["acknowledged_state_id"].is_null(), "{value}");
                assert_eq!(file["durability"], "not-confirmed", "{value}");
                assert!(
                    file["reasons"].to_string().contains(if brush {
                        "brush source"
                    } else {
                        "captured source project"
                    }),
                    "{value}"
                );
            },
        );
        assert_eq!(fs::read(&output).unwrap(), original_target);
        assert_eq!(fs::read(&moved).unwrap(), original_target);
        if brush {
            assert_eq!(fs::read(&project).unwrap(), original);
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn extensionless_dds_stream_fails_closed_without_a_captured_regular_identity() {
    let root = std::env::temp_dir().join(format!("paintcraft-dds-stream-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let input = root.join("extensionless");
    let output = root.join("output.pcnd");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&input)
            .status()
            .unwrap()
            .success()
    );
    fs::write(&output, b"existing output").unwrap();
    let writer_path = input.clone();
    let writer = std::thread::spawn(move || fs::write(writer_path, DDS).unwrap());
    let reply = run(vec![
        "paintcraft-cli".into(),
        "import".into(),
        input.to_str().unwrap().into(),
        "-o".into(),
        output.to_str().unwrap().into(),
        "--overwrite".into(),
        "--frontend".into(),
        "studio".into(),
        "--allow-loss".into(),
        "dds-other-mips-not-imported".into(),
    ]);
    writer.join().unwrap();
    assert_ne!(reply.exit, 0);
    assert!(reply.stderr.contains("captured regular source"));
    assert_eq!(fs::read(&output).unwrap(), b"existing output");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn actual_native_project_fifo_snapshot_publishes_without_checkpoint_reread() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-project-stream-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root).unwrap();
    let input = root.join("input.pcnd");
    let output = root.join("flattened.pcnd");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&input)
            .status()
            .unwrap()
            .success()
    );
    let doc = new_document(2, 1, Some(Rgba([19, 47, 91, 255])), 0, POLICY).unwrap();
    let bytes = pcnd::write(&doc).unwrap();
    let expected_sha = hex(&Sha256::digest(&bytes));
    let writer_path = input.clone();
    // One writer supplies one snapshot. A second checkpoint read would block.
    let writer = std::thread::spawn(move || fs::write(writer_path, bytes).unwrap());
    let reply = run(vec![
        "paintcraft-cli".into(),
        "flatten".into(),
        input.to_str().unwrap().into(),
        "-o".into(),
        output.to_str().unwrap().into(),
        "--expect-input-sha256".into(),
        expected_sha.clone(),
    ]);
    writer.join().unwrap();
    assert_eq!(reply.exit, 0, "{}", reply.stderr);
    let value: serde_json::Value = serde_json::from_slice(&reply.stdout).unwrap();
    assert_eq!(value["file"]["source_sha256"], expected_sha);
    assert_eq!(value["file"]["replacement"], "replaced");
    let result = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(result.canvas, (2, 1));
    let raster = &result.layers[&result.roots[0]].raster;
    assert_eq!(raster.get(0, 0), Rgba([19, 47, 91, 255]));
    assert_eq!(raster.get(1, 0), Rgba([19, 47, 91, 255]));
    fs::remove_dir_all(root).unwrap();
}
