use super::*;
use paintcraft_document::new_document;
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
    assert_eq!(hex(&sink.hash.finalize()), sha(&expected));
    assert!(
        sink.largest <= CHUNK_TARGET,
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
fn empty_documents_have_no_data_members() {
    let doc = new_document(3, 3, Some(Rgba([1, 2, 3, 4])), 0, POLICY).unwrap();
    let bytes = write(&doc).unwrap();
    let names: Vec<_> = zip::read(&bytes, 10, |_| Some(1 << 20))
        .unwrap()
        .into_iter()
        .map(|m| m.0)
        .collect();
    assert_eq!(names, ["document.json", "manifest.json"]);
    assert!(read(&bytes).unwrap().same_semantics(&doc));
}

fn rewrite(bytes: &[u8], mut edit: impl FnMut(&str, Vec<u8>) -> Vec<u8>) -> Vec<u8> {
    let mut members = zip::read(bytes, 100, |_| Some(1 << 24)).unwrap();
    for (n, d) in &mut members {
        *d = edit(n, std::mem::take(d));
    }
    zip::write_stored(&members)
}

/// Replace text in document.json and refresh the manifest so only the edit differs.
fn mutate_document(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let mut doc = None;
    let tmp = rewrite(bytes, |n, d| {
        if n == "document.json" {
            doc = Some(String::from_utf8(d.clone()).unwrap().replacen(from, to, 1));
            doc.clone().unwrap().into_bytes()
        } else {
            d
        }
    });
    let doc = doc.unwrap();
    rewrite(&tmp, |n, d| {
        if n == "manifest.json" {
            let m = String::from_utf8(d).unwrap();
            let old = m
                .split("\"document_bytes\":")
                .nth(1)
                .unwrap()
                .split(',')
                .next()
                .unwrap()
                .to_owned();
            let old_sha = m
                .split("\"document_sha256\":\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .to_owned();
            m.replace(&old_sha, &sha(doc.as_bytes()))
                .replace(
                    &format!("\"document_bytes\":{old}"),
                    &format!("\"document_bytes\":{}", doc.len()),
                )
                .into_bytes()
        } else {
            d
        }
    })
}

#[test]
fn hostile_variants_are_refused_with_stable_codes() {
    let good = specimen();
    let code = |b: &[u8]| read(b).unwrap_err().code;
    // Corrupt block bytes.
    assert_eq!(
        code(&rewrite(&good, |n, mut d| {
            if n.starts_with("data/") {
                d[0] ^= 1;
            }
            d
        })),
        ErrorCode::CodecFailed
    );
    // Extra member.
    let mut members = zip::read(&good, 100, |_| Some(1 << 20)).unwrap();
    members.push(("zzz.txt".into(), vec![]));
    assert_eq!(code(&zip::write_stored(&members)), ErrorCode::CodecFailed);
    // Unknown required feature and unknown schema are capability errors, never editable.
    assert_eq!(
        code(&rewrite(&good, |n, d| if n == "manifest.json" {
            String::from_utf8(d)
                .unwrap()
                .replace("layers.flat", "layers.cmyk")
                .into_bytes()
        } else {
            d
        })),
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        code(&rewrite(&good, |n, d| if n == "manifest.json" {
            String::from_utf8(d)
                .unwrap()
                .replace("document/v1", "document/v2")
                .into_bytes()
        } else {
            d
        })),
        ErrorCode::CapabilityUnsupported
    );
    // Edits that keep the manifest consistent.
    assert_eq!(
        code(&mutate_document(
            &good,
            "\"opacity_u8\":255",
            "\"opacity_u8\":256"
        )),
        ErrorCode::CodecFailed
    );
    assert_eq!(
        code(&mutate_document(&good, "\"block_y\":0", "\"block_y\":5")),
        ErrorCode::CodecFailed
    );
    assert_eq!(
        code(&mutate_document(
            &good,
            "\"decoded_bytes\":16",
            "\"decoded_bytes\":20"
        )),
        ErrorCode::CodecFailed
    );
    assert_eq!(
        code(&mutate_document(&good, "\"offset\":0", "\"offset\":8")),
        ErrorCode::CodecFailed
    );
    assert_eq!(
        code(&mutate_document(
            &good,
            "\"encoding\":\"raw\"",
            "\"encoding\":\"zstd\""
        )),
        ErrorCode::CodecFailed
    );
    assert_eq!(
        code(&mutate_document(
            &good,
            "paintcraft.core/default-v1",
            "paintcraft.core/other-v1"
        )),
        ErrorCode::PolicyUnknown
    );
    assert_eq!(
        code(&mutate_document(
            &good,
            "\"roots\":[\"22222222-2222-4222-8222-222222222222\"]",
            "\"roots\":[]"
        )),
        ErrorCode::CodecFailed
    );
    assert_eq!(
        code(&mutate_document(
            &good,
            "\"kind\":\"raster\"",
            "\"kind\":\"group\""
        )),
        ErrorCode::CapabilityUnsupported
    );
    // Document tampering without refreshing the manifest.
    assert_eq!(
        code(&rewrite(&good, |n, d| if n == "document.json" {
            String::from_utf8(d)
                .unwrap()
                .replacen("Specimen", "Specimeo", 1)
                .into_bytes()
        } else {
            d
        })),
        ErrorCode::CodecFailed
    );
    assert_eq!(code(b"PK"), ErrorCode::CodecFailed);
}
