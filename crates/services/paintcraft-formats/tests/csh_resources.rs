use paintcraft_formats::csh;
use std::io::Write;

#[test]
fn owned_csh_assets_have_exact_records_after_fresh_export() {
    let directory = std::env::var_os("PAINTCRAFT_CSH_EXPORT_DIR").map(std::path::PathBuf::from);
    if let Some(path) = &directory {
        // Explicit opt-in qualification output must never replace old evidence.
        std::fs::create_dir(path).unwrap();
    }
    for (name, bytes) in [
        (
            "empty.csh",
            include_bytes!("csh-fixtures/empty.csh").as_slice(),
        ),
        (
            "paths.csh",
            include_bytes!("csh-fixtures/paths.csh").as_slice(),
        ),
    ] {
        let source = csh::decode(bytes, &|| Ok(())).unwrap();
        let output = csh::encode(&source, &|| Ok(())).unwrap();
        assert_eq!(csh::decode(&output, &|| Ok(())).unwrap(), source);
        if name == "paths.csh" {
            assert_eq!(source.shapes().len(), 5);
            assert_eq!(source.shapes()[0].name, "形 / Shape 🚀");
            assert_eq!(source.shapes()[0].id, source.shapes()[1].id);
            assert_eq!(source.shapes()[4].bounds, [14, 24, 118, 228]);
            assert!(source.shapes().iter().all(|s| s.records.len() == 9));
        }
        if let Some(path) = &directory {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path.join(name))
                .unwrap();
            file.write_all(&output).unwrap();
        }
    }
}
