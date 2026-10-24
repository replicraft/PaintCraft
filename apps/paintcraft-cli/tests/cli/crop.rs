use super::*;

#[test]
fn canvas_crop_applies_after_paint_and_roundtrips_ids_alpha_and_hidden_rgb() {
    let s = Scratch::new("crop");
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
            "24",
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
        {{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":6,"y":7,"width":4,"height":3}},"color":"#FF0000FF","mode":"replace"}},
        {{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":7,"y":8,"width":1,"height":1}},"color":"#17355700","mode":"replace"}},
        {{"op":"canvas.crop","rect":{{"x":4,"y":5,"width":10,"height":8}}}}
    ]}}"##)).unwrap();
    let applied = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
    let doc = paintcraft_formats::pcnd::read(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(doc.canvas, (10, 8));
    assert_eq!(doc.roots[0].to_string(), layer);
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(3, 3).0,
        [0x17, 0x35, 0x57, 0]
    );
    assert_eq!(run(&["render", &out, "-o", &image]).code, 3);
    assert!(!Path::new(&image).exists());
    assert_eq!(
        run(&["render", &out, "-o", &image, "--allow-loss", "hidden-rgb"]).code,
        0
    );
    let (w, h, pixels) = decode_png(&image);
    assert_eq!((w, h), (10, 8));
    assert_eq!(&pixels[(2 * 10 + 2) * 4..][..4], [255, 0, 0, 255]);
    assert_eq!(&pixels[..4], [255; 4]);
}
