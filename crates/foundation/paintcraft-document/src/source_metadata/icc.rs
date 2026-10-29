//! Structural ICC validation only. No curve/LUT interpretation or CMS claim.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IccHeader {
    pub declared_bytes: u32,
    pub version: u32,
    pub profile_class: [u8; 4],
    pub data_color_space: [u8; 4],
    pub connection_space: [u8; 4],
    pub rendering_intent: u32,
    pub tag_count: u32,
}
pub fn inspect_icc(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<IccHeader> {
    check()?;
    let header = bytes
        .get(..132)
        .ok_or_else(|| invalid("Truncated ICC header"))?;
    let word = |at| u32::from_be_bytes(header[at..at + 4].try_into().unwrap());
    let size = word(0);
    let tags = word(128);
    let end = 132u64 + u64::from(tags) * 12;
    if bytes.len() as u64 != u64::from(size) || end > u64::from(size) || &header[36..40] != b"acsp"
    {
        return Err(invalid("ICC header/tag table extent"));
    }
    for entry in bytes[132..end as usize].as_chunks::<12>().0 {
        check()?;
        let offset = u64::from(u32::from_be_bytes(entry[4..8].try_into().unwrap()));
        let length = u64::from(u32::from_be_bytes(entry[8..12].try_into().unwrap()));
        if offset < end || offset + length > u64::from(size) {
            return Err(invalid("ICC tag outside declared payload"));
        }
    }
    check()?;
    Ok(IccHeader {
        declared_bytes: size,
        version: word(8),
        profile_class: header[12..16].try_into().unwrap(),
        data_color_space: header[16..20].try_into().unwrap(),
        connection_space: header[20..24].try_into().unwrap(),
        rendering_intent: word(64),
        tag_count: tags,
    })
}
