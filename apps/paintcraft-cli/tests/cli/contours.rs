use super::*;

#[test]
fn apply_contours_roundtrips_pcnd_and_renders_open_and_closed_paths() {
    let s = Scratch::new("contours");
    let base = s.path("base.pcnd");
    let ops = s.path("ops.json");
    let out = s.path("out.pcnd");
    let image = s.path("out.png");
    assert_eq!(
        run(&[
            "new",
            "--width",
            "32",
            "--height",
            "32",
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
    fs::write(&ops,format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[
        {{"op":"raster.contour","layer_ref":{{"id":"{layer}"}},"points":[{{"x":2,"y":2}},{{"x":12,"y":2}},{{"x":12,"y":12}},{{"x":2,"y":12}}],"closed":true,"fill":"#FF0000FF","stroke":{{"color":"#00000080","size":1}},"mode":"source-over"}},
        {{"op":"raster.contour","layer_ref":{{"id":"{layer}"}},"points":[{{"x":20,"y":2}},{{"x":28,"y":2}},{{"x":28,"y":12}},{{"x":20,"y":12}}],"closed":false,"stroke":{{"color":"#0000FFFF","size":1}},"mode":"source-over"}}
    ]}}"##)).unwrap();
    let applied = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
    assert_eq!(run(&["render", &out, "-o", &image]).code, 0);
    let (w, h, pixels) = decode_png(&image);
    assert_eq!((w, h), (32, 32));
    let at = |x: usize, y: usize| &pixels[(y * 32 + x) * 4..][..4];
    assert_eq!(at(7, 7), [255, 0, 0, 255]);
    assert_eq!(at(12, 12), [127, 127, 127, 255]);
    assert_eq!(at(28, 7), [0, 0, 255, 255]);
    assert_eq!(at(20, 7), [255; 4], "an open path has no closing edge");
}
