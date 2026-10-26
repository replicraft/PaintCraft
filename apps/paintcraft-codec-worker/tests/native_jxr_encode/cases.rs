use super::*;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("paintcraft-jxr-encode-oracle-{}", Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path).unwrap().write_all(bytes).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pixels(kind: &str, width: u32, height: u32) -> Vec<u8> {
    let fixed = match kind {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([255, 0, 0]),
        "green" => Some([0, 255, 0]),
        "blue" => Some([0, 0, 255]),
        _ => None,
    };
    if let Some(color) = fixed {
        return color.repeat((width * height) as usize);
    }
    let mut noise = 0x5043_4a58_5201_u64;
    let mut bytes = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            for channel in 0..3 {
                bytes.push(match kind {
                    "gradient" => ((x * 47 + y * 13 + channel * 71) % 256) as u8,
                    "edge" => {
                        if (x + y + channel) & 1 == 0 {
                            0
                        } else {
                            255
                        }
                    }
                    "noise-lcg" => {
                        noise = noise
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        (noise >> 56) as u8
                    }
                    "markers" => ((x * (channel + 1) + y * 19 + channel * 83) % 256) as u8,
                    _ => panic!("unknown owned vector"),
                });
            }
        }
    }
    bytes
}
#[test]
#[ignore = "requires explicit pinned PAINTCRAFT_JXR_REFERENCE C JXRLib oracle"]
fn actual_child_all14_fresh_rgb_vectors_match_independent_c_jxrlib() {
    let oracle =
        PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_REFERENCE").expect("pinned C oracle"));
    assert!(oracle.is_absolute());
    assert_eq!(
        hex(&Sha256::digest(fs::read(&oracle).unwrap())),
        "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce"
    );
    let scratch = Scratch::new();
    for (kind, w, h) in [
        ("black", 1, 1),
        ("white", 1, 1),
        ("red", 1, 1),
        ("green", 1, 1),
        ("blue", 1, 1),
        ("gradient", 4, 3),
        ("gradient", 17, 19),
        ("markers", 257, 129),
        ("edge", 16, 16),
        ("edge", 17, 16),
        ("edge", 16, 17),
        ("gradient", 1, 17),
        ("gradient", 17, 1),
        ("noise-lcg", 31, 17),
    ] {
        let source = pixels(kind, w, h);
        let before = source.clone();
        let request = wire::Request::capture(
            wire::Layout::new(w, h, wire::ENCODED_BYTES_MAX).unwrap(),
            Arc::new(source),
            owner(),
            &|| Ok(()),
        )
        .unwrap();
        let output = run(&request, &|| Ok(())).unwrap();
        assert!(output.status.success());
        let frame = wire::decode_response(
            &output.bytes,
            request.header(),
            request.header().owner(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(request.rgb(), before);
        let encoded = scratch.write(&format!("{kind}-{w}x{h}.jxr"), frame.bytes());
        let decoded = scratch.0.join(format!("{kind}-{w}x{h}.rgb"));
        let args = [
            "decode".into(),
            w.to_string().into(),
            h.to_string().into(),
            encoded.as_os_str().to_owned(),
            decoded.as_os_str().to_owned(),
        ];
        let c = process::run(
            &oracle,
            &args,
            vec![],
            Limits {
                input_bytes: 1,
                output_bytes: 4096,
                deadline: Duration::from_secs(10),
                resource_exit_code: None,
            },
            &|| Ok(()),
        )
        .unwrap();
        assert!(
            c.status.success(),
            "C decode {kind}-{w}x{h}: {:?}",
            c.status
        );
        assert_eq!(
            fs::read(decoded).unwrap(),
            before,
            "whole-file C pixels {kind}-{w}x{h}"
        );
        if kind == "gradient" && (w, h) == (4, 3) {
            assert_eq!(
                frame.bytes(),
                include_bytes!(
                    "../../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-4x3.jxr"
                )
            );
        }
        if kind == "gradient" && (w, h) == (17, 19) {
            assert_eq!(
                frame.bytes(),
                include_bytes!(
                    "../../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-17x19.jxr"
                )
            );
        }
        println!(
            "JXR_CHILD {kind}-{w}x{h} raw_sha={} encoded_sha={} encoded_bytes={} heap_requested_peak={} C_exact=true",
            hex(&request.header().rgb_sha256()),
            hex(&frame.sha256()),
            frame.bytes().len(),
            frame.heap_peak()
        );
    }
}
