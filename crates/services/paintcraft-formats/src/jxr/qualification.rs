//! Explicit developer experiment only: no production decoder or fresh writer.
use super::*;
use std::{fs, io::Write, path::PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("paintcraft-jxr-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        Ok(Self(path))
    }
    fn write(&self, bytes: &[u8]) -> std::io::Result<PathBuf> {
        let path = self.0.join("source.jxr");
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path)?.write_all(bytes)?;
        Ok(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "explicit experimental native JXRLib differential qualification; no codec allocator/cancellation confinement"]
fn selected_decoder_matches_independent_jxrlib_rgb8_fixtures() {
    use jpegxr::{BitDepth, ChannelOrder, ColorFormat, PixelInterpretation, SampleFormat};
    for bytes in [
        include_bytes!("../../tests/jxr-fixtures/rgb8-4x3.jxr").as_slice(),
        include_bytes!("../../tests/jxr-fixtures/rgb8-17x19.jxr").as_slice(),
    ] {
        let info = inspect_checked(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        let scratch = Scratch::new().unwrap();
        let path = scratch.write(bytes).unwrap();
        // Avoid Decoder::from_slice/from_reader and encode_to_vec: those APIs
        // create predictable files in the shared temporary directory.
        let mut decoder = jpegxr::Decoder::open(&path).unwrap();
        assert_eq!(decoder.dimensions().unwrap(), (info.width, info.height));
        let format = decoder.pixel_format().unwrap();
        assert_eq!(format.bits_per_pixel(), 24);
        assert_eq!(format.channel_count(), 3);
        assert_eq!(format.bits_per_sample(), 8);
        assert_eq!(format.sample_count(), 3);
        assert_eq!(format.bit_depth(), BitDepth::Eight);
        assert_eq!(format.channel_order(), ChannelOrder::Rgb);
        assert_eq!(format.color_format(), ColorFormat::Rgb);
        assert_eq!(format.interpretation(), PixelInterpretation::Rgb);
        assert_eq!(format.sample_format(), SampleFormat::UnsignedInteger);
        assert!(!format.has_alpha());
        assert!(!format.premultiplied_alpha());
        let stride = usize::try_from(info.width).unwrap().checked_mul(3).unwrap();
        let len = stride.checked_mul(info.height as usize).unwrap();
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(len).unwrap();
        pixels.resize(len, 0);
        decoder.decode_into(&mut pixels, stride).unwrap();
        let expected: Vec<u8> = (0..info.height)
            .flat_map(|y| {
                (0..info.width).flat_map(move |x| {
                    (0..3).map(move |c| ((x * 47 + y * 13 + c * 71) % 256) as u8)
                })
            })
            .collect();
        assert_eq!(
            pixels, expected,
            "C JXRLib encoded and independently decoded this exact authored gradient"
        );
    }
}
