//! Independent byte fixtures for bounded extended raster conversion.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::image::{self, ImageFormat};

fn qoi(width: u32, height: u32, channels: u8, body: &[u8]) -> Vec<u8> {
    let mut bytes = b"qoif".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([channels, 0]);
    bytes.extend(body);
    bytes.extend([0, 0, 0, 0, 0, 0, 0, 1]);
    bytes
}
#[test]
fn qoi_rgb_rgba_diff_luma_index_run_and_informative_channels_keep_exact_samples() {
    let bytes = qoi(5, 1, 4, &[0xff, 10, 20, 30, 40, 0x7f, 0xa1, 0x88, 12, 0xc0]);
    let decoded = image::decode(&bytes).unwrap();
    assert_eq!(decoded.format, ImageFormat::Qoi);
    assert_eq!(
        decoded.rgba,
        [
            10, 20, 30, 40, 11, 21, 31, 40, 12, 22, 32, 40, 10, 20, 30, 40, 10, 20, 30, 40
        ]
    );
    assert!(decoded.losses.is_empty());
    // The QOI channel field is informative; an RGBA opcode in an RGB-labeled
    // stream still carries its actual alpha, including hidden RGB.
    for channels in [3, 4] {
        let decoded = image::decode(&qoi(1, 1, channels, &[0xff, 10, 20, 30, 0])).unwrap();
        assert_eq!(decoded.rgba, [10, 20, 30, 0]);
    }
    assert_eq!(
        image::decode(&qoi(1, 1, 3, &[0xfe, 20, 30, 40]))
            .unwrap()
            .rgba,
        [20, 30, 40, 255]
    );
}
#[test]
fn qoi_refuses_truncation_extra_images_linear_color_and_overlong_runs_before_allocation() {
    let valid = qoi(1, 1, 4, &[0xff, 10, 20, 30, 40]);
    for end in 0..valid.len() {
        assert!(image::decode(&valid[..end]).is_err(), "prefix {end}");
    }
    let mut extra = valid.clone();
    extra.push(0);
    assert!(image::decode(&extra).is_err());
    assert!(image::decode(&qoi(1, 1, 4, &[0xfd])).is_err());
    let mut linear = valid;
    linear[13] = 1;
    assert_eq!(
        image::decode(&linear).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        image::decode_with_pixel_limit(&qoi(1024, 1024, 4, &[]), 1)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
fn farbfeld(width: u32, height: u32, samples: &[u16]) -> Vec<u8> {
    let mut bytes = b"farbfeld".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    for sample in samples {
        bytes.extend(sample.to_be_bytes());
    }
    bytes
}
#[test]
fn farbfeld_retains_network_order_high_bytes_alpha_and_explicit_precision_loss() {
    let bytes = farbfeld(
        2,
        1,
        &[0x1234, 0xabcd, 0x0102, 0x8000, 0x1111, 0x2222, 0x3333, 0],
    );
    let decoded = image::decode(&bytes).unwrap();
    assert_eq!(decoded.format, ImageFormat::Farbfeld);
    assert_eq!(decoded.rgba, [0x12, 0xab, 1, 0x80, 0x11, 0x22, 0x33, 0]);
    assert_eq!(decoded.losses, ["depth-16-reduced-to-8"]);
    for end in 0..bytes.len() {
        assert!(image::decode(&bytes[..end]).is_err());
    }
    let mut extra = bytes;
    extra.push(0);
    assert!(image::decode(&extra).is_err());
    assert_eq!(
        image::decode_with_pixel_limit(&farbfeld(1024, 1024, &[]), 1)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn pnm_ascii_binary_bitmaps_gray_rgb_and_pam_alpha_have_independent_pixels() {
    let fixtures:[(&[u8],&[u8]);7]=[
        (b"P1\n3 1\n0 1 0\n",&[255,255,255,255,0,0,0,255,255,255,255,255]),
        (b"P2\n2 1\n255\n0 127\n",&[0,0,0,255,127,127,127,255]),
        (b"P3\n1 1\n255\n20 30 40\n",&[20,30,40,255]),
        (b"P4\n3 1\n\x40",&[255,255,255,255,0,0,0,255,255,255,255,255]),
        (b"P5\n2 1\n255\n\x00\x7f",&[0,0,0,255,127,127,127,255]),
        (b"P6\n1 1\n255\n\x14\x1e\x28",&[20,30,40,255]),
        (b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n\x14\x1e\x28\x80",&[20,30,40,128]),
    ];
    for (bytes, expected) in fixtures {
        let decoded = image::decode(bytes).unwrap();
        assert_eq!(decoded.format, ImageFormat::Pnm);
        assert_eq!(decoded.rgba, expected, "{bytes:?}");
        assert!(decoded.losses.is_empty());
    }
}
#[test]
fn pnm_16bit_precision_comments_and_grayscale_alpha_are_explicit() {
    let decoded =
        image::decode(b"P6\n# synthetic fixture\n1 1\n65535\n\x12\x34\xab\xcd\x01\x02").unwrap();
    assert_eq!(decoded.rgba, [0x12, 0xab, 1, 255]);
    assert_eq!(
        decoded.losses,
        ["depth-16-reduced-to-8", "pnm-metadata-ignored"]
    );
    let decoded=image::decode(b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 65535\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n\x12\x34\x80\x00").unwrap();
    assert_eq!(decoded.rgba, [0x12, 0x12, 0x12, 0x80]);
    assert_eq!(decoded.losses, ["depth-16-reduced-to-8"]);
}
#[test]
fn pnm_rejects_concatenation_header_bombs_invalid_samples_and_unsupported_tuple_models() {
    for bytes in [
        &b"P6\n1 1\n255\n\x00\x00"[..],
        &b"P6\n1 1\n255\n\x00\x00\x00\x00"[..],
        &b"P1\n1 1\n2\n"[..],
        &b"P1\n1 1\n0\nP1\n1 1\n0\n"[..],
        &b"P7\nWIDTH 1\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n\0\0\0\0"[..],
        &b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE CMYK\nENDHDR\n\0\0\0\0"[..],
    ] {assert!(image::decode(bytes).is_err(),"{bytes:?}");}
    assert_eq!(
        image::decode_with_pixel_limit(b"P6\n1000000 1000000\n255\n", 1)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut header = b"P6\n#".to_vec();
    header.extend(vec![b'x'; 65536]);
    header.extend(b"\n1 1\n255\n\0\0\0");
    assert_eq!(
        image::decode(&header).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        image::decode(b"P5\n1 1\n31\n\x1f").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
fn pcx_header(width: u16, height: u16, planes: u8) -> Vec<u8> {
    let mut h = vec![0; 128];
    h[..4].copy_from_slice(&[10, 5, 1, 8]);
    h[8..10].copy_from_slice(&(width - 1).to_le_bytes());
    h[10..12].copy_from_slice(&(height - 1).to_le_bytes());
    h[65] = planes;
    h[66..68].copy_from_slice(&width.next_multiple_of(2).to_le_bytes());
    h
}
#[test]
fn pcx_v5_rgb_and_indexed_odd_width_rows_decode_via_selected_codec() {
    let mut rgb = pcx_header(3, 1, 3);
    rgb.extend([10, 20, 30, 0, 40, 50, 60, 0, 70, 80, 90, 0]);
    let decoded = image::decode(&rgb).unwrap();
    assert_eq!(
        decoded.rgba,
        [10, 40, 70, 255, 20, 50, 80, 255, 30, 60, 90, 255]
    );
    assert_eq!(decoded.losses, ["pcx-metadata-ignored"]);
    let mut indexed = pcx_header(3, 1, 1);
    indexed.extend([0, 1, 2, 0, 12]);
    let mut palette = vec![0; 768];
    palette[..9].copy_from_slice(&[10, 40, 70, 20, 50, 80, 30, 60, 90]);
    indexed.extend(palette);
    let decoded = image::decode(&indexed).unwrap();
    assert_eq!(
        decoded.rgba,
        [10, 40, 70, 255, 20, 50, 80, 255, 30, 60, 90, 255]
    );
    assert_eq!(
        decoded.losses,
        ["pcx-metadata-ignored", "palette-indices-expanded"]
    );
    for end in 0..indexed.len() {
        assert!(image::decode(&indexed[..end]).is_err(), "prefix {end}");
    }
}
#[test]
fn pcx_refuses_runs_crossing_planes_zero_runs_truncation_and_unqualified_variants() {
    let mut rgb = pcx_header(1, 1, 3);
    rgb.extend([0xc3, 8, 0, 0, 0, 0]);
    assert!(image::decode(&rgb).is_err());
    let mut zero = pcx_header(1, 1, 3);
    zero.extend([0xc0, 8, 0, 0, 0, 0]);
    assert!(image::decode(&zero).is_err());
    let mut short = pcx_header(1, 1, 3);
    short.extend([1, 0, 2, 0, 3]);
    assert!(image::decode(&short).is_err());
    let mut old = pcx_header(1, 1, 3);
    old[1] = 2;
    old.extend([1, 0, 2, 0, 3, 0]);
    assert_eq!(
        image::decode(&old).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        image::decode_with_pixel_limit(&pcx_header(1024, 1024, 3), 1)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn pcx_v5_packed_and_planar_palette_profiles_handle_odd_widths() {
    for (bits, planes, packed, expected) in [
        (1, 1, vec![0x40, 0], vec![0, 1, 0]),
        (2, 1, vec![0x18, 0], vec![0, 1, 2]),
        (4, 1, vec![0x01, 0x20], vec![0, 1, 2]),
        (1, 2, vec![0x40, 0, 0x20, 0], vec![0, 1, 2]),
        (1, 3, vec![0x40, 0, 0x20, 0, 0x20, 0], vec![0, 1, 6]),
        (
            1,
            4,
            vec![0x40, 0, 0x20, 0, 0x20, 0, 0x20, 0],
            vec![0, 1, 14],
        ),
    ] {
        let mut bytes = pcx_header(3, 1, planes);
        bytes[3] = bits;
        bytes[66..68].copy_from_slice(&2u16.to_le_bytes());
        for index in 0..16 {
            bytes[16 + index * 3..19 + index * 3].copy_from_slice(&[
                index as u8 * 10,
                index as u8,
                index as u8 * 3,
            ]);
        }
        if bits == 1 && planes == 1 {
            bytes[16..22].copy_from_slice(&[0, 0, 0, 255, 255, 255]);
        }
        bytes.extend(packed);
        let decoded = image::decode(&bytes).unwrap();
        let expected = expected
            .into_iter()
            .flat_map(|index| {
                if bits == 1 && planes == 1 {
                    if index == 0 {
                        [0, 0, 0, 255]
                    } else {
                        [255, 255, 255, 255]
                    }
                } else {
                    [index * 10, index, index * 3, 255]
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(decoded.rgba, expected, "bits={bits}, planes={planes}");
        assert_eq!(
            decoded.losses,
            ["pcx-metadata-ignored", "palette-indices-expanded"]
        );
    }
    let mut color_mono = pcx_header(1, 1, 1);
    color_mono[3] = 1;
    color_mono.extend([0, 0]);
    assert_eq!(
        image::decode(&color_mono).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
#[ignore = "requires separately installed FFmpeg; validates pixels with independent decoders"]
fn ffmpeg_independently_reopens_the_qualified_qoi_pnm_and_pcx_fixtures() {
    use std::process::{Command, Stdio};
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch(
        std::env::temp_dir().join(format!("paintcraft-extended-oracle-{}", std::process::id())),
    );
    std::fs::create_dir_all(&scratch.0).unwrap();
    let mut rgb = pcx_header(3, 1, 3);
    rgb.extend([10, 20, 30, 0, 40, 50, 60, 0, 70, 80, 90, 0]);
    let mut fixtures=vec![
        ("qoi",qoi(2,1,4,&[0xff,10,20,30,40,0xc0])),
        ("pbm",b"P4\n3 1\n\x40".to_vec()),
        ("pgm",b"P5\n2 1\n255\n\x00\x7f".to_vec()),
        ("ppm",b"P6\n1 1\n255\n\x14\x1e\x28".to_vec()),
        ("pam",b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n\x14\x1e\x28\x80".to_vec()),
        ("pcx",rgb),
    ];
    for (bits, planes) in [(1, 1), (2, 1), (4, 1), (8, 1), (1, 2), (1, 3), (1, 4)] {
        fixtures.push(("pcx", pcx_indexed_fixture(9, 3, bits, planes).0));
    }
    for (codec, bytes) in fixtures {
        let expected = image::decode(&bytes).unwrap().rgba;
        let ffmpeg = std::env::var_os("PAINTCRAFT_FFMPEG").unwrap_or_else(|| "ffmpeg".into());
        let input = scratch.0.join(format!("fixture.{codec}"));
        std::fs::write(&input, &bytes).unwrap();
        let output = Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(&input)
            .args([
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .output()
            .expect("install FFmpeg or set PAINTCRAFT_FFMPEG");
        assert!(
            output.status.success(),
            "{codec}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected, "independent {codec} decoder");
    }
}

fn pcx_indexed_fixture(width: u16, height: u16, bits: u8, planes: u8) -> (Vec<u8>, Vec<u8>) {
    let mut bytes = pcx_header(width, height, planes);
    bytes[3] = bits;
    let proper = (u32::from(width) * u32::from(bits)).div_ceil(8) as usize;
    let lane = proper.next_multiple_of(2);
    bytes[66..68].copy_from_slice(&(lane as u16).to_le_bytes());
    let colors = 1usize << (u32::from(bits) * u32::from(planes));
    let mut palette = vec![0u8; colors * 3];
    for i in 0..colors {
        let color = if colors == 2 {
            if i == 0 { [0; 3] } else { [255; 3] }
        } else {
            [i as u8, 255 - i as u8, i as u8 ^ 85]
        };
        palette[i * 3..i * 3 + 3].copy_from_slice(&color);
    }
    for i in 0..colors.min(16) {
        bytes[16 + i * 3..19 + i * 3].copy_from_slice(&palette[i * 3..i * 3 + 3]);
    }
    let mut expected = Vec::new();
    for y in 0..usize::from(height) {
        let indices = (0..usize::from(width))
            .map(|x| (x + y * 7) % colors)
            .collect::<Vec<_>>();
        for index in &indices {
            expected.extend(&palette[index * 3..index * 3 + 3]);
            expected.push(255);
        }
        for plane in 0..planes {
            let mut row = vec![0u8; lane];
            for (x, index) in indices.iter().enumerate() {
                let value = if planes == 1 {
                    *index as u8
                } else {
                    (*index >> plane) as u8 & 1
                };
                let bit = x * usize::from(bits);
                row[bit / 8] |= value << (8 - usize::from(bits) - bit % 8);
            }
            for value in row {
                if value >= 192 {
                    bytes.extend([0xc1, value]);
                } else {
                    bytes.push(value);
                }
            }
        }
    }
    if colors == 256 {
        bytes.push(12);
        bytes.extend(palette);
    }
    (bytes, expected)
}
#[test]
fn pcx_odd_width_multirow_packed_and_planar_layouts_keep_all_palette_indices() {
    for width in [1, 7, 9, 257] {
        for (bits, planes) in [(1, 1), (2, 1), (4, 1), (8, 1), (1, 2), (1, 3), (1, 4)] {
            let (bytes, expected) = pcx_indexed_fixture(width, 3, bits, planes);
            assert_eq!(
                image::decode(&bytes).unwrap().rgba,
                expected,
                "width={width}, bits={bits}, planes={planes}"
            );
        }
    }
}
