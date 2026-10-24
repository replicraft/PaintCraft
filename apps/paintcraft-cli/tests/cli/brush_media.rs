use super::*;
use paintcraft_core::types::brush::BrushMedia;
#[test]
fn every_brush_medium_applies_through_cli_and_renders_a_distinct_image() {
    let s = Scratch::new("media");
    let base = s.path("base.pcnd");
    let ops = s.path("ops.json");
    let out = s.path("out.pcnd");
    let image = s.path("out.png");
    assert_eq!(
        run(&[
            "new",
            "--width",
            "64",
            "--height",
            "48",
            "--background",
            "#FFFFFFFF",
            "-o",
            &base
        ])
        .code,
        0
    );
    let info = run(&["inspect", &base, "--json"]).json();
    let state = text(&info, &["ticket", "state_id"]);
    let layer = info
        .get("document")
        .unwrap()
        .get("layers")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .get("id")
        .unwrap()
        .as_str()
        .unwrap();
    let mut outputs = std::collections::BTreeSet::new();
    for media in BrushMedia::ALL {
        fs::write(&ops, format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[{{"op":"raster.stroke_path","layer_ref":{{"id":"{layer}"}},"points":[{{"x":2560,"y":5120}},{{"x":12544,"y":7680}}],"point_scale":256,"brush":"{}","size":16,"color":"#0078D480","mode":"source-over"}}]}}"##, media.id())).unwrap();
        let applied = run(&[
            "apply",
            &base,
            "--operations",
            &ops,
            "-o",
            &out,
            "--json",
            "--overwrite",
        ]);
        assert_eq!(
            applied.code, 0,
            "{media:?}: {} {}",
            applied.stdout, applied.stderr
        );
        let rendered = run(&["render", &out, "-o", &image, "--overwrite"]);
        assert_eq!(rendered.code, 0, "{} {}", rendered.stdout, rendered.stderr);
        let (w, h, pixels) = decode_png(&image);
        assert_eq!((w, h), (64, 48));
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| *pixel != [255; 4])
        );
        assert!(outputs.insert(pixels), "{media:?}");
    }
}
