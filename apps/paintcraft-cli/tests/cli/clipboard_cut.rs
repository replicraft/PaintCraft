use super::*;
use paintcraft_core::types::Rgba;
#[test]
fn real_cli_cut_removal_preserves_input_and_exact_unselected_pixels() {
    let scratch = Scratch::new("clipboard-cut");
    let source = scratch.path("source.pcnd");
    let operations = scratch.path("cut.json");
    let destination = scratch.path("cut.pcnd");
    let doc =
        paintcraft_document::import_image(3, 2, &[17, 44, 99, 0].repeat(6), "cli-cut").unwrap();
    let id = doc.roots[0];
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    fs::write(&source, &original).unwrap();
    let ops = format!(
        r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"0"}},"operations":[{{"op":"selection.set_rect","rect":{{"x":1,"y":0,"width":1,"height":2}}}},{{"op":"raster.cut_selection","layer_ref":{{"id":"{id}"}},"clear":"#FFFFFFFF"}}]}}"##,
        doc.state_id
    );
    fs::write(&operations, &ops).unwrap();
    let applied = run(&[
        "apply",
        &source,
        "--operations",
        &operations,
        "-o",
        &destination,
        "--json",
    ]);
    assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
    let reopened = paintcraft_formats::pcnd::read(&fs::read(&destination).unwrap()).unwrap();
    assert_eq!(reopened.canvas, doc.canvas);
    assert_eq!(reopened.document_id, doc.document_id);
    assert_eq!(reopened.roots, doc.roots);
    for y in 0..2 {
        for x in 0..3 {
            assert_eq!(
                reopened.layers[&id].raster.get(x, y),
                if x == 1 {
                    Rgba([255; 4])
                } else {
                    Rgba([17, 44, 99, 0])
                }
            );
        }
    }
    assert_eq!(fs::read(&source).unwrap(), original);
    let saved = fs::read(&destination).unwrap();
    fs::write(
        &operations,
        ops.replace("paintcraft.batch/v2", "paintcraft.batch/v1"),
    )
    .unwrap();
    let rejected = run(&[
        "apply",
        &source,
        "--operations",
        &operations,
        "-o",
        &destination,
        "--json",
        "--overwrite",
    ]);
    assert_eq!(rejected.code, 2, "{} {}", rejected.stdout, rejected.stderr);
    assert_eq!(text(&rejected.json(), &["engine", "code"]), "input.schema");
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(fs::read(&destination).unwrap(), saved);
    let v1 = run(&["schema", "batch"]);
    let v2 = run(&["schema", "batch-v2"]);
    assert_eq!((v1.code, v2.code), (0, 0));
    assert!(!v1.stdout.contains("raster.cut_selection"));
    assert!(v2.stdout.contains("raster.cut_selection"));
}
