//! PNG output of the flat render: 8-bit straight RGBA, no ancillary metadata.

#[cfg(test)]
use paintcraft_core::types::ErrorCode;
use paintcraft_core::types::Result;

pub fn encode_rgba8(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    crate::image::encode(crate::image::ImageFormat::Png, width, height, rgba, 90)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_exact_samples_including_zero_alpha() {
        let px = [255, 0, 0, 255, 0, 255, 0, 128, 7, 8, 9, 0, 1, 2, 3, 4];
        let bytes = encode_rgba8(2, 2, &px).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let mut reader = ::png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!(
            (info.width, info.height, info.color_type),
            (2, 2, ::png::ColorType::Rgba)
        );
        assert_eq!(&buf[..16], &px);
    }

    #[test]
    fn rejects_mismatched_buffers() {
        assert_eq!(
            encode_rgba8(2, 2, &[0; 15]).unwrap_err().code,
            ErrorCode::InvariantViolation
        );
    }
}
