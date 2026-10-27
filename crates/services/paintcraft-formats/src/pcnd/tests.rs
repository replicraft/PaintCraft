use super::*;
use paintcraft_document::{POLICY, hex, new_document};
use paintcraft_editor::session::Session;
use std::path::PathBuf;

fn specimen() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../conformance/minimal.pcnd"),
    )
    .unwrap()
}

#[test]
fn streaming_output_matches_canonical_bytes_under_partial_writes() {
    struct Sink {
        hash: Sha256,
        bytes: usize,
        largest: usize,
    }
    impl std::io::Write for Sink {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            self.largest = self.largest.max(input.len());
            let count = input.len().min(1024);
            self.hash.update(&input[..count]);
            self.bytes += count;
            Ok(count)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let width = 2048;
    let height = 768;
    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|pixel| [(pixel % 251) as u8, 45, 83, 255])
        .collect();
    let doc =
        paintcraft_document::import_image(width, height, &pixels, "streaming-fixture").unwrap();
    let expected = write(&doc).unwrap();
    let mut sink = Sink {
        hash: Sha256::new(),
        bytes: 0,
        largest: 0,
    };
    write_to(&doc, &mut sink).unwrap();
    assert_eq!(sink.bytes, expected.len());
    assert_eq!(hex(&sink.hash.finalize()), hex(&Sha256::digest(&expected)));
    assert!(
        sink.largest <= BLOCK_BYTES_MAX as usize,
        "payload chunks must obey sink backpressure"
    );
    assert!(read(&expected).unwrap().same_semantics(&doc));
}

#[test]
fn failed_streaming_sink_does_not_change_the_source() {
    struct Full;
    impl std::io::Write for Full {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "injected disk full",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let bytes = specimen();
    let doc = read(&bytes).unwrap();
    let error = write_to(&doc, Full).unwrap_err();
    assert_eq!(error.code, ErrorCode::StorageFailed);
    assert_eq!(write(&doc).unwrap(), bytes);
}

#[test]
fn specimen_reads_and_the_canonical_writer_reproduces_it_exactly() {
    let bytes = specimen();
    let doc = read(&bytes).unwrap();
    assert_eq!(doc.canvas, (2, 2));
    let layer = &doc.layers[&doc.roots[0]];
    let mut samples = Vec::new();
    layer.raster.for_each_row(|r| samples.extend_from_slice(r));
    assert_eq!(
        hex(&samples),
        "ff0000000".to_owned() + "0ff00ff0000ff80ffffffff"
    );
    assert_eq!(write(&doc).unwrap(), bytes);
}

#[test]
fn written_files_round_trip_with_hidden_rgb_titles_and_unicode() {
    let mut s = Session::new(new_document(300, 2, None, 0, POLICY).unwrap());
    let t = s.ticket();
    let batch = format!(
        r##"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[
          {{"op":"layer.add","ref":"a","name":"Ébauche ✏️ \"q\"","width":300,"height":2,"origin":{{"x":-3,"y":0}},"default_pixel":"#0000FFFF"}},
          {{"op":"raster.fill_rect","layer_ref":{{"ref":"a"}},"rect":{{"x":250,"y":1,"width":50,"height":1}},"color":"#AB000000","mode":"replace"}}]}}"##,
        t.state_id, t.publication_epoch
    );
    s.apply_json(batch.as_bytes()).unwrap();
    let mut doc = s.document().clone();
    doc.title = Some("Titre".into());
    let bytes = write(&doc).unwrap();
    let back = read(&bytes).unwrap();
    assert!(back.same_semantics(&doc) && back.state_id == doc.state_id);
    assert_eq!(
        write(&back).unwrap(),
        bytes,
        "writing twice is byte-identical"
    );
    let id = back.roots[0];
    assert_eq!(back.layers[&id].raster.get(280, 1), Rgba([0xAB, 0, 0, 0]));
}

#[test]
fn empty_documents_need_no_pixel_payload() {
    let doc = new_document(3, 3, Some(Rgba([1, 2, 3, 4])), 0, POLICY).unwrap();
    let bytes = write(&doc).unwrap();
    assert!(bytes.starts_with(MAGIC));
    assert!(bytes.len() < 160);
    assert_eq!(bytes, write_compact(&doc).unwrap());
    assert!(read(&bytes).unwrap().same_semantics(&doc));
}

#[test]
fn caller_layer_bound_refuses_before_records_without_narrowing_legacy_readers() {
    let mut doc = new_document(1, 1, Some(Rgba([17, 51, 93, 255])), 0, POLICY).unwrap();
    for _ in 0..8 {
        let mut layer = doc.layers[&doc.roots[0]].clone();
        layer.id = Uuid::new_v4();
        doc.roots.push(layer.id);
        doc.layers.insert(layer.id, layer);
    }
    let bytes = write(&doc).unwrap();
    assert!(read(&bytes).unwrap().same_semantics(&doc));
    assert!(read_with_limits(&bytes, 1, 0).unwrap().same_semantics(&doc));
    assert!(
        read_with_limits_checked(&bytes, 1, 0, &|| Ok(()))
            .unwrap()
            .same_semantics(&doc)
    );
    assert!(
        read_with_layer_limit_checked(&bytes, 1, 0, 9, &|| Ok(()))
            .unwrap()
            .same_semantics(&doc)
    );
    let error = read_with_layer_limit_checked(&bytes, 1, 0, 8, &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.message.contains("host layer limit"));
    assert_eq!(
        read_with_layer_limit_checked(&bytes, 1, 0, LAYER_COUNT_MAX + 1, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        read_with_layer_limit_checked(&bytes, 1, 0, 8, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
