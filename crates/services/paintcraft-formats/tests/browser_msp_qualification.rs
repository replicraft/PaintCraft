//! Opt-in owned production specimens, not a guided campaign or public gate.
use paintcraft_core::types::{
    ErrorCode, Rgba,
    json::{self, Value},
};
use paintcraft_document::{Document, POLICY, hex, new_document};
use paintcraft_formats::{codec_worker::Owner, msp::web_export as wire, pcnd};
use paintcraft_raster::{Admission, RasterLayer};
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::Path};
use uuid::Uuid;
fn id(value: u128) -> Uuid {
    Uuid::from_u128(0x1111_1111_1111_4111_8111_0000_0000_0000 + value)
}
fn write(root: &Path, name: &str, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(name))
        .unwrap()
        .write_all(bytes)
        .unwrap();
}
fn pixels(width: u32, height: u32, pattern: u8) -> Vec<u8> {
    let mut result = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let byte = if pattern == 0 {
                match y {
                    0 => 0,
                    1 => 255,
                    _ => (x / 8) as u8,
                }
            } else if pattern == 1 {
                if (x / 8 + y / 8) % 2 == 0 { 0 } else { 255 }
            } else {
                (u64::from(x / 8) * 37 + u64::from(y) * 13) as u8
            };
            let c = if byte & (0x80 >> (x % 8)) == 0 {
                0
            } else {
                255
            };
            result.extend([c, c, c, 255]);
        }
    }
    result
}
fn document(index: u128, width: u32, height: u32, rgba: &[u8]) -> Document {
    let mut doc = new_document(width, height, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    doc.document_id = id(0x100000 + index);
    doc.state_id = id(0x200000 + index);
    doc.title = Some("Owned browser MSP qualification".into());
    let mut layer = doc.layers.remove(&doc.roots[0]).unwrap();
    layer.id = id(0x500000 + index * 16);
    layer.name = "Visible source".into();
    layer.raster = RasterLayer::from_rgba(
        width,
        height,
        Rgba([0, 0, 0, 0]),
        rgba,
        &mut Admission::new(wire::RESIDENT_BYTES_MAX / 4),
    )
    .unwrap();
    doc.roots = vec![layer.id];
    doc.layers.insert(layer.id, layer);
    doc
}
fn extra(doc: &mut Document, index: u128, hidden: bool, count: usize) {
    for n in 1..=count {
        let mut layer = doc.layers[&doc.roots[0]].clone();
        layer.id = id(0x500000 + index * 16 + n as u128);
        layer.name = format!("Owned layer {n}");
        if hidden {
            layer.raster = RasterLayer::new(doc.canvas.0, doc.canvas.1, Rgba([128, 0, 0, 0]));
        }
        doc.roots.push(layer.id);
        doc.layers.insert(layer.id, layer);
    }
}
#[test]
#[ignore = "explicit owned production fixture emission; requires fresh PAINTCRAFT_MSP_WEB_OUTPUT"]
fn emit_owned_browser_msp_specimens() {
    let root = std::env::var_os("PAINTCRAFT_MSP_WEB_OUTPUT").unwrap();
    let root = Path::new(&root);
    std::fs::create_dir_all(root).unwrap();
    let cases = [
        ("ui-layered", 64, 64, 1, 1, 0),
        ("golden-runs-2048", 2048, 3, 0, 0, 0),
        ("odd-9x3", 9, 3, 2, 0, 0),
        ("boundary-square", 2048, 2048, 2, 0, 0),
        ("boundary-row-table", 65, 64527, 2, 0, 0),
        ("boundary-wide", 65535, 1, 2, 0, 0),
        ("boundary-tall", 1, 65535, 2, 0, 0),
        ("boundary-raw-two-layers", 2048, 2048, 2, 2, 0),
        ("refuse-gray", 4, 3, 2, 0, 1),
        ("refuse-transparency", 4, 3, 2, 0, 2),
        ("refuse-nine-layers", 4, 3, 2, 3, 0),
    ];
    let mut rows = Vec::new();
    for (index, (name, width, height, pattern, layers, bad)) in cases.into_iter().enumerate() {
        let index = index as u128;
        let mut rgba = pixels(width, height, pattern);
        if bad == 1 {
            rgba.fill(128);
            for p in rgba.as_chunks_mut::<4>().0 {
                p[3] = 255;
            }
        }
        if bad == 2 {
            for p in rgba.as_chunks_mut::<4>().0 {
                p[3] = 128;
            }
        }
        let mut doc = document(index, width, height, &rgba);
        if layers == 1 {
            extra(&mut doc, index, true, 1);
        } else if layers == 2 {
            extra(&mut doc, index, false, 1);
        } else if layers == 3 {
            extra(&mut doc, index, false, 8);
        }
        let owner = Owner {
            session: id(0x300000 + index),
            state: doc.state_id,
            epoch: 7,
            frontend: 3,
        };
        let nonce = id(0x400000 + index);
        let source = if layers == 3 {
            pcnd::write(&doc).unwrap()
        } else {
            wire::SourcePlan::capture(&doc)
                .unwrap()
                .snapshot(&doc)
                .unwrap()
        };
        let header = if layers == 3 {
            assert_eq!(
                wire::SourcePlan::capture(&doc).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
            wire::Header {
                source_len: source.len() as u32,
                size: doc.canvas,
                owner,
                nonce,
                source_sha256: Sha256::digest(&source).into(),
                source_flags: 1,
            }
        } else {
            wire::Header::capture(
                &wire::SourcePlan::capture(&doc).unwrap(),
                &source,
                owner,
                nonce,
            )
            .unwrap()
        };
        let mut request = header.encode().to_vec();
        request.extend_from_slice(&source);
        let response = wire::execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
        let frame =
            wire::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION")).unwrap();
        write(root, &format!("{name}.pcnd"), &source);
        write(root, &format!("{name}.header"), &header.encode());
        write(root, &format!("{name}.response"), &response);
        let (status, code, encoded_sha) = match frame {
            wire::Frame::Success {
                encoded,
                preview,
                preview_size,
                losses,
            } => {
                assert_eq!(bad, 0);
                assert_ne!(layers, 3);
                assert_eq!(
                    losses.len(),
                    if layers == 1 {
                        2
                    } else {
                        usize::from(layers == 2)
                    }
                );
                for y in 0..preview_size.1 {
                    for x in 0..preview_size.0 {
                        let sx = u64::from(x) * u64::from(width) / u64::from(preview_size.0);
                        let sy = u64::from(y) * u64::from(height) / u64::from(preview_size.1);
                        let src = (sy * u64::from(width) + sx) as usize * 4;
                        let dst = (y * preview_size.0 + x) as usize * 4;
                        assert_eq!(&preview[dst..dst + 4], &rgba[src..src + 4]);
                    }
                }
                if name == "golden-runs-2048" {
                    assert_eq!(
                        encoded,
                        include_bytes!("msp2-writer-fixtures/runs-2048.msp")
                    );
                }
                write(root, &format!("{name}.msp"), encoded);
                write(root, &format!("{name}.rgba"), &rgba);
                ("success", "", hex(&Sha256::digest(encoded)))
            }
            wire::Frame::Refused(error) => {
                let expected = if layers == 3 {
                    ErrorCode::ResourceLimit
                } else {
                    ErrorCode::CapabilityUnsupported
                };
                assert_eq!(error.code, expected);
                (
                    "refused",
                    if layers == 3 {
                        "ResourceLimit"
                    } else {
                        "CapabilityUnsupported"
                    },
                    String::new(),
                )
            }
        };
        let resident: u64 = doc
            .layers
            .values()
            .map(|l| l.raster.resident_tiles() as u64 * RasterLayer::TILE_BYTES as u64)
            .sum();
        if name == "boundary-wide" || name == "boundary-tall" {
            assert_eq!(resident, wire::RESIDENT_BYTES_MAX);
        }
        if name == "boundary-raw-two-layers" {
            assert!(source.len() > 32 << 20);
        }
        rows.push(Value::object([
            ("name", Value::str(name)),
            ("width", Value::Int(i64::from(width))),
            ("height", Value::Int(i64::from(height))),
            ("sourceFlags", Value::Int(i64::from(header.source_flags))),
            ("pattern", Value::Int(i64::from(pattern))),
            ("status", Value::str(status)),
            ("code", Value::str(code)),
            (
                "owner",
                Value::object([
                    ("session", Value::str(owner.session.to_string())),
                    ("state", Value::str(owner.state.to_string())),
                    ("epoch", Value::str(owner.epoch.to_string())),
                    ("frontend", Value::Int(3)),
                ]),
            ),
            ("nonce", Value::str(nonce.to_string())),
            ("sourceBytes", Value::Int(source.len() as i64)),
            ("residentBytes", Value::Int(resident as i64)),
            ("sourceSHA", Value::str(hex(&Sha256::digest(&source)))),
            ("responseSHA", Value::str(hex(&Sha256::digest(&response)))),
            ("encodedSHA", Value::str(encoded_sha)),
            (
                "rgbaSHA",
                Value::str(if status == "success" {
                    hex(&Sha256::digest(&rgba))
                } else {
                    String::new()
                }),
            ),
        ]));
        println!(
            "{name}: {status}, source={} resident={resident}",
            source.len()
        );
    }
    write(root, "cases.json", &json::canonical(&Value::Array(rows)));
}
