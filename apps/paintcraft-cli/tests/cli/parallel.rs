use super::*;

#[test]
fn bounded_parallel_export_matches_serial_and_worker_limits_reject_before_writing() {
    let dir = Scratch::new("parallel-render");
    let doc = dir.path("layered.pcnd");
    let mut session = paintcraft_editor::session::Session::new(
        paintcraft_document::new_document(
            259,
            257,
            Some(paintcraft_core::types::Rgba([17, 45, 82, 255])),
            42,
            paintcraft_document::POLICY,
        )
        .unwrap(),
    );
    let ticket = session.ticket();
    let batch = format!(
        r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"layer.add","ref":"top","name":"Top","width":260,"height":260,"origin":{{"x":-5,"y":3}},"default_pixel":"#FF000080"}}]}}"##,
        ticket.state_id, ticket.publication_epoch
    );
    session.apply_json(batch.as_bytes()).unwrap();
    fs::write(
        &doc,
        paintcraft_formats::pcnd::write(session.document()).unwrap(),
    )
    .unwrap();
    let serial = dir.path("serial.png");
    let a = run(&[
        "--json",
        "render",
        &doc,
        "--output",
        &serial,
        "--allow-loss",
        "flatten",
    ]);
    assert_eq!(a.code, 0, "{} {}", a.stdout, a.stderr);
    let (w, h, expected) = decode_png(&serial);
    assert_eq!((w, h), (259, 257));
    // Hand-calculated alpha-128 red over the opaque base, with finite clipping.
    assert_eq!(
        &expected[(3 * 259) * 4..(3 * 259) * 4 + 4],
        &[136, 22, 41, 255]
    );
    assert_eq!(&expected[..4], &[17, 45, 82, 255]);
    for workers in ["2", "4"] {
        let output = dir.path(&format!("workers-{workers}.png"));
        let b = run(&[
            "--json",
            "render",
            &doc,
            "--output",
            &output,
            "--allow-loss",
            "flatten",
            "--render-workers",
            workers,
        ]);
        assert_eq!(b.code, 0, "{} {}", b.stdout, b.stderr);
        assert_eq!(decode_png(&output).2, expected);
    }
    for workers in ["0", "65"] {
        let output = dir.path(&format!("refused-{workers}.png"));
        assert_ne!(
            run(&[
                "render",
                &doc,
                "--output",
                &output,
                "--render-workers",
                workers
            ])
            .code,
            0
        );
        assert!(!Path::new(&output).exists());
    }
}
