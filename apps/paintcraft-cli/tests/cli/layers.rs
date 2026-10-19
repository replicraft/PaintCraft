use super::*;

#[test]
fn duplicate_layer_round_trips_through_real_cli_without_changing_source_pixels() {
    let s = Scratch::new("duplicate-layer");
    let (base, ops, out, png) = (
        s.path("base.pcnd"),
        s.path("ops.json"),
        s.path("copy.pcnd"),
        s.path("copy.png"),
    );
    assert_eq!(
        run(&[
            "new",
            "--width",
            "2",
            "--height",
            "1",
            "--background",
            "#123456FF",
            "-o",
            &base
        ])
        .code,
        0
    );
    let ins = run(&["inspect", &base, "--json"]).json();
    let state = text(&ins, &["ticket", "state_id"]);
    let source = ins
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
      {{"op":"layer.duplicate","layer_ref":{{"id":"{source}"}},"ref":"copy","name":"Independent copy"}},
      {{"op":"raster.fill_rect","layer_ref":{{"ref":"copy"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#FF0000FF","mode":"replace"}}
    ]}}"##)).unwrap();
    let applied = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
    let reopened = run(&["inspect", &out, "--json"]).json();
    let layers = reopened
        .get("document")
        .unwrap()
        .get("layers")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(layers.len(), 2);
    assert_eq!(
        layers[1].get("name").unwrap().as_str(),
        Some("Independent copy")
    );
    assert_ne!(layers[0].get("id"), layers[1].get("id"));
    assert_eq!(
        run(&["render", &out, "-o", &png, "--allow-loss", "flatten"]).code,
        0
    );
    assert_eq!(
        decode_png(&png).2,
        vec![255, 0, 0, 255, 0x12, 0x34, 0x56, 255]
    );
    let source_png = s.path("source.png");
    assert_eq!(run(&["render", &base, "-o", &source_png]).code, 0);
    assert_eq!(
        decode_png(&source_png).2,
        vec![0x12, 0x34, 0x56, 255, 0x12, 0x34, 0x56, 255]
    );
}
