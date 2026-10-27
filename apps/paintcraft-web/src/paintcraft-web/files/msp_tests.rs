use super::*;
#[test]
fn default_msp1_route_keeps_exact_bytes_and_full_canvas_capacity() {
    for (width, height) in [(9u32, 3u32), (2048, 2048)] {
        let doc = paintcraft_document::new_document(
            width,
            height,
            Some(paintcraft_core::types::Rgba([0, 0, 0, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let before = pcnd::write(&doc).unwrap();
        let actual = encode(&doc, Some(ImageExport::Msp)).unwrap();
        // Independent fresh-header/opaque-black MSP1 bytes; no Worker, RLE,
        // threshold, alpha removal or profile narrowing belongs to this route.
        let mut words = [0u16; 16];
        words[0] = 0x6144;
        words[1] = 0x4d6e;
        words[2] = width as u16;
        words[3] = height as u16;
        words[4..8].fill(1);
        words[8] = width as u16;
        words[9] = height as u16;
        words[12] = words.iter().fold(0, |x, w| x ^ w);
        let mut expected: Vec<_> = words.into_iter().flat_map(u16::to_le_bytes).collect();
        expected.resize(32 + width.div_ceil(8) as usize * height as usize, 0);
        assert_eq!(actual, expected);
        assert_eq!(before, pcnd::write(&doc).unwrap());
    }
}
