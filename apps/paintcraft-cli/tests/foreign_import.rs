//! Real CLI foreign conversion protects originals and requires named loss consent.
use paintcraft_formats::{pcnd, project_import};
use std::{fs, path::Path, process::Command};
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn foreign_layered_import_keeps_layers_and_never_replaces_the_source() {
    let dir =
        std::env::temp_dir().join(format!("paintcraft-foreign-import-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/adapters");
    for (name, fixture, losses, count) in [
        (
            "source.paint",
            root.join("paint-lib/tests/fixtures/mspaintlib-sample.paint"),
            project_import::PAINT_LOSSES.join(","),
            5,
        ),
        (
            "source.pdn",
            root.join("pdn-lib/tests/fixtures/paintdotjs-2-layer.pdn"),
            project_import::PDN_LOSS.into(),
            2,
        ),
        (
            "source.ora",
            root.join("../services/paintcraft-formats/tests/ora-fixtures/deflated.ora"),
            "ora-resolution-ignored".into(),
            3,
        ),
        (
            "source.psd",
            root.join("../services/paintcraft-formats/tests/psd-fixtures/layers-3.psd"),
            "psd-metadata-ignored".into(),
            2,
        ),
        (
            "source.psb",
            root.join("../services/paintcraft-formats/tests/psd-fixtures/layers-1.psb"),
            "psd-metadata-ignored".into(),
            2,
        ),
    ] {
        let source = dir.join(name);
        let dest = source.with_file_name(format!("{name}.pcnd"));
        let bytes = fs::read(fixture).unwrap();
        fs::write(&source, &bytes).unwrap();
        let a = source.to_str().unwrap();
        let b = dest.to_str().unwrap();
        assert!(!run(&["import", a, "-o", b]).status.success());
        assert!(!dest.exists());
        let output = run(&["import", a, "-o", b, "--allow-loss", &losses, "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let loaded = pcnd::read(&fs::read(&dest).unwrap()).unwrap();
        assert_eq!(loaded.roots.len(), count);
        assert!(
            !run(&["import", a, "-o", a, "--overwrite", "--allow-loss", &losses])
                .status
                .success()
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
    fs::remove_dir_all(dir).unwrap();
}
fn pcx(rgb: [u8; 3]) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    bytes[..4].copy_from_slice(&[10, 5, 1, 8]);
    bytes[65] = 3;
    bytes[66..68].copy_from_slice(&2u16.to_le_bytes());
    for sample in rgb {
        if sample >= 192 {
            bytes.extend([0xc1, sample]);
        } else {
            bytes.push(sample);
        }
        bytes.push(0);
    }
    bytes
}
#[test]
fn dcx_requires_an_explicit_page_and_reopens_exact_selected_pixels() {
    let dir = std::env::temp_dir().join(format!("paintcraft-dcx-import-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("pages.dcx");
    let dest = dir.join("page.pcnd");
    let mut bytes = vec![0; 4100];
    bytes[..4].copy_from_slice(&987_654_321u32.to_le_bytes());
    for (index, color) in [[10, 20, 30], [200, 210, 220]].into_iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        bytes.extend(pcx(color));
    }
    fs::write(&source, &bytes).unwrap();
    let a = source.to_str().unwrap();
    let b = dest.to_str().unwrap();
    let losses = "pcx-metadata-ignored,dcx-other-pages-not-imported";
    assert!(
        !run(&["import", a, "-o", b, "--allow-loss", losses])
            .status
            .success()
    );
    assert!(!dest.exists());
    let result = run(&["import", a, "-o", b, "--page", "1", "--allow-loss", losses]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let doc = pcnd::read(&fs::read(dest).unwrap()).unwrap();
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(0, 0).0,
        [200, 210, 220, 255]
    );
    assert_eq!(fs::read(source).unwrap(), bytes);
    fs::remove_dir_all(dir).unwrap();
}
