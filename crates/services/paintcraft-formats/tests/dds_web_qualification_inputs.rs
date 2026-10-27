//! Optional browser proof input curation; one independent frozen DDS control.
use paintcraft_document::Document;
use paintcraft_formats::{
    codec_worker::Owner,
    dds::{self, web_export as wire},
    pcnd,
};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|n| format!("{n:02x}"))
        .collect()
}
fn pixels(w: u32, h: u32, opaque: bool) -> Vec<u8> {
    (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| {
                [
                    ((x * 23 + y * 7) % 256) as u8,
                    ((x * 9 + y * 31 + 13) % 256) as u8,
                    ((x * 17 + y * 11 + 71) % 256) as u8,
                    if opaque {
                        255
                    } else {
                        [0, 51, 127, 204, 255][((x + y) % 5) as usize]
                    },
                ]
            })
        })
        .collect()
}
fn document(w: u32, h: u32, opaque: bool) -> Document {
    paintcraft_document::import_image(w, h, &pixels(w, h, opaque), "DDS browser qualification")
        .unwrap()
}
fn selected(encoding: dds::Encoding, transfer: dds::Transfer, high: bool) -> dds::TextureOptions {
    dds::TextureOptions {
        encoding,
        transfer,
        alpha: if encoding == dds::Encoding::Bc1 {
            dds::SourceAlpha::Opaque
        } else {
            dds::SourceAlpha::Straight
        },
        mips: dds::MipCount::Full,
        filter: dds::MipFilter::BoxStored,
        quality: if high && encoding != dds::Encoding::Rgba8 {
            dds::TextureQuality::High
        } else {
            dds::TextureQuality::Normal
        },
    }
}
fn emit(
    out: &Path,
    name: &str,
    doc: &Document,
    options: dds::TextureOptions,
    golden: bool,
) -> String {
    let plan = wire::SourcePlan::capture(doc, options).unwrap();
    let source = plan.snapshot(doc).unwrap();
    let owner = Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 1,
        frontend: 3,
    };
    let header = wire::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
    let mut request = header.encode().to_vec();
    request.extend(&source);
    let start = std::time::Instant::now();
    let response = wire::execute(&request, "0.1.0", &|| Ok(())).unwrap();
    let (status, encoded, preview) =
        match wire::decode_response(&response, &header, owner, "0.1.0").unwrap() {
            wire::Frame::Success {
                encoded, preview, ..
            } => ("success", encoded.to_vec(), preview.to_vec()),
            wire::Frame::Refused(error) => {
                println!("{name}: {error:?}");
                ("refused", vec![], vec![])
            }
        };
    if golden {
        assert_eq!(
            encoded,
            include_bytes!("dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-BoxStored.dds")
        );
        assert_ne!(
            encoded,
            include_bytes!("dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-Nearest.dds")
        );
    }
    fs::write(out.join(format!("{name}.pcnd")), &source).unwrap();
    fs::write(out.join(format!("{name}.request")), &request).unwrap();
    fs::write(out.join(format!("{name}.response")), &response).unwrap();
    fs::write(out.join(format!("{name}.dds")), &encoded).unwrap();
    fs::write(out.join(format!("{name}.preview")), &preview).unwrap();
    let settings = plan.settings().map(|n| n.to_string()).join(",");
    format!(
        "{{\"name\":\"{name}\",\"status\":\"{status}\",\"width\":{},\"height\":{},\"sourceBytes\":{},\"sourceSHA\":\"{}\",\"encodedSHA\":\"{}\",\"responseSHA\":\"{}\",\"settings\":[{settings}],\"sourceFlags\":{},\"owner\":{{\"session\":\"{}\",\"state\":\"{}\",\"epoch\":1,\"frontend\":3}},\"nonce\":\"{}\",\"nativePreparationMs\":{}}}",
        doc.canvas.0,
        doc.canvas.1,
        source.len(),
        hash(&source),
        hash(&encoded),
        hash(&response),
        plan.source_flags(),
        owner.session,
        owner.state,
        doc.document_id,
        start.elapsed().as_millis()
    )
}
#[test]
#[ignore = "local candidate qualification fixture emission, no timing claim"]
fn emit_browser_requests() {
    let out = std::path::PathBuf::from(
        std::env::var_os("PAINTCRAFT_DDS_BROWSER_INPUTS").expect("explicit output"),
    );
    fs::create_dir(&out).unwrap();
    let mut cases = Vec::new();
    let opaque = document(7, 5, true);
    let alpha = document(7, 5, false);
    for (name, doc, encoding, transfer, high, golden) in [
        (
            "golden-bc1-normal-srgb-box",
            &opaque,
            dds::Encoding::Bc1,
            dds::Transfer::Srgb,
            false,
            true,
        ),
        (
            "small-rgba-linear-box",
            &alpha,
            dds::Encoding::Rgba8,
            dds::Transfer::Linear,
            false,
            false,
        ),
        (
            "small-bc2-srgb-box",
            &alpha,
            dds::Encoding::Bc2,
            dds::Transfer::Srgb,
            false,
            false,
        ),
        (
            "small-bc3-linear-high-box",
            &alpha,
            dds::Encoding::Bc3,
            dds::Transfer::Linear,
            true,
            false,
        ),
        (
            "refuse-bc1-transparent",
            &alpha,
            dds::Encoding::Bc1,
            dds::Transfer::Srgb,
            true,
            false,
        ),
    ] {
        cases.push(emit(
            &out,
            name,
            doc,
            selected(encoding, transfer, high),
            golden,
        ));
    }
    for (name, opaque, encoding, transfer) in [
        (
            "boundary-rgba-srgb-box",
            false,
            dds::Encoding::Rgba8,
            dds::Transfer::Srgb,
        ),
        (
            "boundary-bc1-srgb-high-box",
            true,
            dds::Encoding::Bc1,
            dds::Transfer::Srgb,
        ),
        (
            "boundary-bc2-srgb-high-box",
            false,
            dds::Encoding::Bc2,
            dds::Transfer::Srgb,
        ),
        (
            "boundary-bc3-linear-high-box",
            false,
            dds::Encoding::Bc3,
            dds::Transfer::Linear,
        ),
    ] {
        let doc = document(2048, 2048, opaque);
        cases.push(emit(
            &out,
            name,
            &doc,
            selected(encoding, transfer, true),
            false,
        ));
    }
    let long = document(65536, 16, false);
    assert!(
        wire::SourcePlan::capture(
            &long,
            selected(dds::Encoding::Bc3, dds::Transfer::Srgb, true)
        )
        .is_err(),
        "1MP long-strip padded tiles exceed the explicit 24MiB residency profile"
    );
    assert!(
        wire::SourcePlan::capture(
            &document(2049, 2048, true),
            selected(dds::Encoding::Bc1, dds::Transfer::Srgb, true)
        )
        .is_err()
    );
    fs::write(out.join("cases.json"), format!("[{}]\n", cases.join(",\n"))).unwrap();
    // Real source writer is used; the independently qualified compressed golden
    // proves one exact host filter mapping, not general BC color-error bounds.
    assert_eq!(
        pcnd::read(&pcnd::write(&opaque).unwrap()).unwrap().canvas,
        (7, 5)
    );
}

fn append(doc: &mut Document, other: Document, index: u128) {
    let mut layer = other.layers.values().next().unwrap().clone();
    layer.id = uuid::Uuid::from_u128(1000 + index);
    layer.name = "𐀀".repeat(1024);
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
}
#[test]
#[ignore = "local candidate max-source fixture, not a broad performance campaign"]
fn emit_resource_boundary() {
    let out = std::path::PathBuf::from(
        std::env::var_os("PAINTCRAFT_DDS_BROWSER_INPUTS").expect("explicit output"),
    );
    fs::create_dir(&out).unwrap();
    let mut doc = document(2048, 2048, true);
    doc.title = Some("𐀀".repeat(4096));
    doc.layers.values_mut().next().unwrap().name = "𐀀".repeat(1024);
    append(&mut doc, document(1024, 2048, false), 2);
    append(
        &mut doc,
        paintcraft_document::new_document(
            1024,
            2047,
            Some(paintcraft_core::types::Rgba([10, 20, 30, 64])),
            3,
            paintcraft_document::POLICY,
        )
        .unwrap(),
        3,
    );
    for i in 4..=8 {
        append(
            &mut doc,
            paintcraft_document::new_document(
                1,
                1,
                Some(paintcraft_core::types::Rgba([123, 2, 34, 240])),
                i,
                paintcraft_document::POLICY,
            )
            .unwrap(),
            u128::from(i),
        );
    }
    assert_eq!(doc.layers.len(), 8);
    let resident = doc
        .layers
        .values()
        .map(|l| {
            l.raster.resident_tiles() as u64 * paintcraft_raster::RasterLayer::TILE_BYTES as u64
        })
        .sum::<u64>();
    assert_eq!(resident, 24 << 20);
    let logical = doc
        .layers
        .values()
        .map(|l| u64::from(l.raster.width()) * u64::from(l.raster.height()))
        .sum::<u64>();
    assert!(logical <= wire::STAGED_LAYER_PIXELS_MAX);
    println!(
        "eight-layer source: {resident} resident bytes, {logical} logical pixels, 4096-character title and 1024-character names"
    );
    let cases = [
        emit(
            &out,
            "boundary-max-source-rgba-box",
            &doc,
            selected(dds::Encoding::Rgba8, dds::Transfer::Srgb, false),
            false,
        ),
        emit(
            &out,
            "boundary-max-source-bc3-linear-high-box",
            &doc,
            selected(dds::Encoding::Bc3, dds::Transfer::Linear, true),
            false,
        ),
    ];
    fs::write(out.join("cases.json"), format!("[{}]\n", cases.join(",\n"))).unwrap();
    append(
        &mut doc,
        paintcraft_document::new_document(
            1,
            1,
            Some(paintcraft_core::types::Rgba([1, 2, 3, 255])),
            9,
            paintcraft_document::POLICY,
        )
        .unwrap(),
        9,
    );
    assert!(
        wire::SourcePlan::capture(
            &doc,
            selected(dds::Encoding::Rgba8, dds::Transfer::Srgb, false)
        )
        .is_err(),
        "nine complete layers must be refused before host serialization"
    );
}
