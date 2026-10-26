//! Test-only derivative of the shared OFL Selawik Bold font. Rename the reserved
//! family in naming records, retain copyright/license records and fix checksums.
pub const TEXT: &[u8] = br#"<svg xmlns='http://www.w3.org/2000/svg' width='128' height='48'><text x='2' y='34' font-family='PaintCF' font-size='30'>Paint</text></svg>"#;

pub fn font() -> Vec<u8> {
    let mut bytes = include_bytes!("../../../../../assets/fonts/selawik/selawkb.ttf").to_vec();
    let count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
    let mut head = 0;
    for index in 0..count {
        let record = 12 + index * 16;
        let offset =
            u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
        if &bytes[record..record + 4] == b"head" {
            head = offset;
        }
        if &bytes[record..record + 4] != b"name" {
            continue;
        }
        let records = usize::from(u16::from_be_bytes(
            bytes[offset + 2..offset + 4].try_into().unwrap(),
        ));
        let strings = offset
            + usize::from(u16::from_be_bytes(
                bytes[offset + 4..offset + 6].try_into().unwrap(),
            ));
        for index in 0..records {
            let at = offset + 6 + index * 12;
            let name_id = u16::from_be_bytes(bytes[at + 6..at + 8].try_into().unwrap());
            if ![1, 3, 4, 6, 16].contains(&name_id) {
                continue;
            }
            let length = usize::from(u16::from_be_bytes(
                bytes[at + 8..at + 10].try_into().unwrap(),
            ));
            let begin = strings
                + usize::from(u16::from_be_bytes(
                    bytes[at + 10..at + 12].try_into().unwrap(),
                ));
            let text = &mut bytes[begin..begin + length];
            for (from, to) in [
                (b"Selawik".as_slice(), b"PaintCF".as_slice()),
                (
                    b"\0S\0e\0l\0a\0w\0i\0k".as_slice(),
                    b"\0P\0a\0i\0n\0t\0C\0F".as_slice(),
                ),
            ] {
                let mut start = 0;
                while let Some(at) = text[start..].windows(from.len()).position(|v| v == from) {
                    let at = start + at;
                    text[at..at + to.len()].copy_from_slice(to);
                    start = at + to.len();
                }
            }
        }
    }
    assert!(head > 0);
    bytes[head + 8..head + 12].fill(0);
    for index in 0..count {
        let record = 12 + index * 16;
        let offset =
            u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
        let length =
            u32::from_be_bytes(bytes[record + 12..record + 16].try_into().unwrap()) as usize;
        let sum = checksum(&bytes[offset..offset + length]);
        bytes[record + 4..record + 8].copy_from_slice(&sum.to_be_bytes());
    }
    let adjustment = 0xb1b0_afbau32.wrapping_sub(checksum(&bytes));
    bytes[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    assert_eq!(checksum(&bytes), 0xb1b0_afba);
    bytes
}
fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}
