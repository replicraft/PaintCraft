use super::*;
#[test]
fn orthogonal_canvas_transforms_roundtrip_every_orientation_and_raw_rgba() {
    let s = Scratch::new("orthogonal");
    let base = s.path("base.pcnd");
    assert_eq!(
        run(&[
            "new",
            "--width",
            "3",
            "--height",
            "2",
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
    let white = [255; 4];
    let hidden = [17, 33, 55, 0];
    let red = [255, 0, 0, 255];
    for (t, w, h, expected) in [
        (
            "rotate-right",
            2,
            3,
            vec![white, red, white, white, hidden, white],
        ),
        (
            "rotate-left",
            2,
            3,
            vec![white, hidden, white, white, red, white],
        ),
        (
            "rotate-180",
            3,
            2,
            vec![hidden, white, white, white, white, red],
        ),
        (
            "flip-horizontal",
            3,
            2,
            vec![white, white, red, hidden, white, white],
        ),
        (
            "flip-vertical",
            3,
            2,
            vec![white, white, hidden, red, white, white],
        ),
    ] {
        let ops = s.path(&format!("{t}.json"));
        let out = s.path(&format!("{t}.pcnd"));
        fs::write(&ops,format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[
            {{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#FF0000FF","mode":"replace"}},
            {{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":2,"y":1,"width":1,"height":1}},"color":"#11213700","mode":"replace"}},
            {{"op":"canvas.transform","transform":"{t}"}}
        ]}}"##)).unwrap();
        let applied = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
        assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
        let doc = paintcraft_formats::pcnd::read(&fs::read(&out).unwrap()).unwrap();
        assert_eq!(doc.canvas, (w, h));
        assert_eq!(doc.roots[0].to_string(), layer);
        let l = &doc.layers[&doc.roots[0]];
        assert_eq!(l.origin, (0, 0));
        let actual: Vec<_> = (0..h)
            .flat_map(|y| (0..w).map(move |x| l.raster.get(x.into(), y.into()).0))
            .collect();
        assert_eq!(actual, expected, "{t}");
    }
}
