use super::*;
#[test]
fn selected_rotation_roundtrips_clear_overlap_transparent_rgb_and_unchanged_canvas() {
    let s = Scratch::new("selection-transform");
    let base = s.path("base.pcnd");
    let ops = s.path("ops.json");
    let out = s.path("out.pcnd");
    assert_eq!(
        run(&[
            "new",
            "--width",
            "8",
            "--height",
            "6",
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
        {{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":2,"y":2,"width":1,"height":1}},"color":"#FF0000FF","mode":"replace"}},
        {{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":4,"y":3,"width":1,"height":1}},"color":"#11213700","mode":"replace"}},
        {{"op":"selection.set_rect","rect":{{"x":2,"y":2,"width":3,"height":2}}}},
        {{"op":"raster.transform_selection","layer_ref":{{"id":"{layer}"}},"transform":"rotate-right","clear":"#FFFFFFFF"}}
    ]}}"##)).unwrap();
    let a = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(a.code, 0, "{} {}", a.stdout, a.stderr);
    let doc = paintcraft_formats::pcnd::read(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(doc.canvas, (8, 6));
    assert_eq!(doc.roots[0].to_string(), layer);
    let l = &doc.layers[&doc.roots[0]];
    assert_eq!(l.raster.get(3, 2).0, [255, 0, 0, 255]);
    assert_eq!(l.raster.get(2, 4).0, [17, 33, 55, 0]);
    assert_eq!(l.raster.get(4, 3).0, [255; 4]);
    assert_eq!(l.raster.get(7, 5).0, [255; 4]);
}
