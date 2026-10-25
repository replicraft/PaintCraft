//! Encoder-only AVIF interoperability, deliberately separate from file admission.
use paintcraft_formats::avif;

/// FFmpeg exposes AVIF alpha as a separate auxiliary AV1 stream. Merge it
/// explicitly rather than accepting the opaque default stream as an alpha oracle.
#[test]
#[ignore = "requires installed FFmpeg with the independent libdav1d decoder"]
fn ffmpeg_libdav1d_reopens_color_and_auxiliary_alpha() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(
        std::env::temp_dir().join(format!("paintcraft-avif-oracle-{}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).unwrap();
    for alpha in [false, true] {
        let (width, height) = (16, 16);
        let source: Vec<u8> = (0..width * height)
            .flat_map(|n| [80, 90, 100, if alpha { n as u8 } else { 255 }])
            .collect();
        let encoded = avif::encode_rgba8(width, height, &source, 100, &|| Ok(())).unwrap();
        let input = directory.0.join("image.avif");
        let output = directory.0.join("image.rgba");
        std::fs::write(&input, &encoded.bytes).unwrap();
        let mut command = std::process::Command::new("ffmpeg");
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-c:v",
                "libdav1d",
                "-i",
            ])
            .arg(&input);
        if alpha {
            command.args(["-filter_complex", "[0:0][0:1]alphamerge"]);
        } else {
            command.args(["-map", "0:0"]);
        }
        let result = command
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let decoded = std::fs::read(&output).unwrap();
        assert_eq!(decoded.len(), source.len());
        for (expected, actual) in source
            .as_chunks::<4>()
            .0
            .iter()
            .zip(decoded.as_chunks::<4>().0.iter())
        {
            assert!(
                actual[3].abs_diff(expected[3]) <= 2,
                "alpha {actual:?} expected {expected:?}"
            );
            if expected[3] != 0 {
                for channel in 0..3 {
                    assert!(
                        actual[channel].abs_diff(expected[channel]) <= 3,
                        "RGB {actual:?} expected {expected:?}"
                    );
                }
            }
        }
        if let Some(path) = std::env::var_os("PAINTCRAFT_AVIF_ORACLE_DIR") {
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(&path).unwrap();
            let stem = if alpha { "alpha" } else { "opaque" };
            std::fs::write(path.join(format!("{stem}.avif")), &encoded.bytes).unwrap();
            std::fs::write(path.join(format!("{stem}.source.rgba")), &source).unwrap();
            std::fs::write(path.join(format!("{stem}.decoded.rgba")), &decoded).unwrap();
        }
        assert!(encoded.losses.contains(&"lossy-avif"));
        assert_eq!(encoded.losses.contains(&"alpha-lossy"), alpha);
    }
}
