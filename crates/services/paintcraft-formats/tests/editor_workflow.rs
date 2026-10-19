//! A complete editing/save/export sequence with independently calculated pixels.
use paintcraft_core::types::{Rgba, json::Value};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_formats::{export, image::ImageFormat, pcnd};
use paintcraft_render::render_flat;

fn apply(session: &mut Session, operations: &str) {
    let ticket = session.ticket();
    session.apply_json(format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{operations}}}"#,
        ticket.state_id, ticket.publication_epoch,
    ).as_bytes()).unwrap();
}

#[test]
fn layered_edit_history_native_reopen_and_independent_png_decode_agree() {
    let mut s = Session::new(new_document(2, 2, Some(Rgba([255; 4])), 15, POLICY).unwrap());
    apply(
        &mut s,
        r##"[
        {"op":"layer.add","ref":"red","name":"Red","width":2,"height":2,"origin":{"x":0,"y":0},"default_pixel":"#00000000"},
        {"op":"raster.fill_rect","layer_ref":{"ref":"red"},"rect":{"x":0,"y":0,"width":2,"height":1},"color":"#FF000080","mode":"replace"},
        {"op":"layer.set_properties","layer_ref":{"ref":"red"},"opacity_u8":128},
        {"op":"layer.add","ref":"blue","name":"Hidden blue","width":2,"height":2,"origin":{"x":0,"y":0},"default_pixel":"#00000000"},
        {"op":"raster.pencil_path","layer_ref":{"ref":"blue"},"points":[{"x":1,"y":1}],"color":"#0000FFFF","mode":"replace"},
        {"op":"layer.set_properties","layer_ref":{"ref":"blue"},"visible":false},
        {"op":"selection.set_rect","rect":{"x":1,"y":0,"width":1,"height":2}}
    ]"##,
    );
    let before = s.snapshot();
    let red = s.document().roots[1];
    let op = Value::object([
        ("op", Value::str("raster.fill_rect")),
        (
            "layer_ref",
            Value::object([("id", Value::str(red.to_string()))]),
        ),
        (
            "rect",
            Value::object([
                ("x", Value::Int(0)),
                ("y", Value::Int(0)),
                ("width", Value::Int(2)),
                ("height", Value::Int(2)),
            ]),
        ),
        ("color", Value::str("#00FF0080")),
        ("mode", Value::str("replace")),
    ]);
    apply(
        &mut s,
        &String::from_utf8(paintcraft_core::types::json::canonical(&Value::Array(
            vec![op],
        )))
        .unwrap(),
    );
    // Effective alpha floor((128*128+127)/255)=64. Over white, the
    // unpainted channels are exactly 191, calculated independently of the kernel.
    let expected = [
        255, 191, 191, 255, 191, 255, 191, 255, 255, 255, 255, 255, 191, 255, 191, 255,
    ];
    assert_eq!(render_flat(s.document()).unwrap(), expected);
    let after = s.snapshot();
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
    s.redo().unwrap();
    assert!(s.document().same_semantics(&after));
    let bytes = pcnd::write(s.document()).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-backend-workflow-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let native = dir.join("edited.pcnd");
    assert!(paintcraft_formats::save::save(&native, &bytes, false).is_success());
    let reopened = pcnd::read(&std::fs::read(native).unwrap()).unwrap();
    assert!(reopened.same_semantics(&after));
    assert_eq!(reopened.state_id, after.state_id);
    assert_eq!(render_flat(&reopened).unwrap(), expected);
    let encoded =
        export::export_flat(&reopened, ImageFormat::Png, 90, &["flatten".into()]).unwrap();
    let png_path = dir.join("export.png");
    assert!(paintcraft_formats::save::save(&png_path, &encoded.bytes, false).is_success());
    let mut reader = png::Decoder::new(std::io::Cursor::new(std::fs::read(png_path).unwrap()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (2, 2));
    assert_eq!(&pixels[..info.buffer_size()], expected);
    std::fs::remove_dir_all(dir).unwrap();
}
