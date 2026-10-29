use super::*;
use paintcraft_core::types::json;
use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires artifacts from the actual production browser orientation proof"]
fn actual_browser_orientation_pcnds_match_independent_coordinate_pixels_and_stable_ids() {
    let root = std::path::PathBuf::from(
        std::env::var("PAINTCRAFT_TIFF_ORIENTATION_ARTIFACTS")
            .expect("actual browser attempt folder"),
    );
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/tiff-orientation-fixtures");
    let oracle = json::parse(&std::fs::read(fixtures.join("oracle.json")).unwrap()).unwrap();
    let mut identities = std::collections::BTreeMap::new();
    for orientation in 1..=8 {
        let source = format!("gray-alpha16-be-orientation-{orientation}.tif");
        let fixture = oracle
            .get("fixtures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v.get("file").unwrap().as_str() == Some(&source))
            .unwrap();
        let original = std::fs::read(fixtures.join(&source)).unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&original)),
            fixture.get("sha256").unwrap().as_str().unwrap()
        );
        let page = &fixture.get("pages").unwrap().as_array().unwrap()[0];
        let rgba: Vec<u8> = page
            .get("rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect();
        let width = page.get("width").unwrap().as_i64().unwrap() as u32;
        let height = page.get("height").unwrap().as_i64().unwrap() as u32;
        for suffix in ["", "-reopened"] {
            let name = format!("orientation-{orientation}{suffix}.pcnd");
            let bytes = std::fs::read(root.join(&name)).unwrap();
            let document = paintcraft_formats::pcnd::read_with_limits(
                &bytes,
                wire::PIXELS_MAX.into(),
                paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            )
            .unwrap();
            assert_eq!(document.canvas, (width, height));
            assert_eq!(document.roots.len(), 1);
            let raster = &document.layers[&document.roots[0]].raster;
            for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
                assert_eq!(
                    raster
                        .get(
                            (index % width as usize) as i64,
                            (index / width as usize) as i64
                        )
                        .0,
                    *pixel,
                    "{name}: pixel{index}"
                );
            }
            let identity = (
                document.document_id,
                document.state_id,
                document.roots.clone(),
            );
            if suffix.is_empty() {
                assert!(identities.insert(orientation, identity).is_none());
            } else {
                assert_eq!(identity, identities[&orientation]);
            }
        }
    }
    assert_eq!(
        identities
            .values()
            .map(|v| v.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
    );
}
