//! Browser-only whole-source TIFF operations, separate from decoder2 codecs.
use crate::codec_worker::Owner;
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const VERSION: u16 = 1;
pub const REQUEST_HEADER: usize = 176;
pub const RESPONSE_HEADER: usize = 200;
pub const SOURCE_MAX: usize = 64 << 20;
pub const PIXELS_MAX: u32 = 4_194_304;
pub const SIDE_MAX: u32 = paintcraft_core::types::policy::CANVAS_SIDE_MAX;
pub const PAGES_MAX: u16 = 1024;
pub const ROW_BYTES: usize = 24;
pub const REPORT_BYTES: usize = 256;
pub const OUTPUT_MAX: usize = RESPONSE_HEADER + 64 + REPORT_BYTES + PIXELS_MAX as usize * 4;
const NONE: u16 = u16::MAX;
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub index: u16,
    pub width: u32,
    pub height: u32,
    pub ifd_offset: u64,
    pub supported: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub page: Page,
    pub page_count: u16,
    pub source_sha256: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub source_bytes: u64,
    pub source_sha256: [u8; 32],
    pub owner: Owner,
    pub nonce: Uuid,
    pub pixels: u32,
    pub selection: Option<Selection>,
}
impl Header {
    pub fn capture(
        source: &[u8],
        owner: Owner,
        nonce: Uuid,
        pixels: u32,
        selection: Option<Selection>,
    ) -> Result<Self> {
        let value = Self {
            source_bytes: source.len() as u64,
            source_sha256: digest(source),
            owner,
            nonce,
            pixels,
            selection,
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        if self.source_bytes == 0
            || self.source_bytes > SOURCE_MAX as u64
            || self.pixels == 0
            || self.pixels > PIXELS_MAX
            || self.owner.frontend != 3
        {
            return Err(invalid("TIFF browser request limits/frontend"));
        }
        if let Some(selected) = self.selection {
            let page = selected.page;
            if !page.supported
                || selected.page_count == 0
                || selected.page_count > PAGES_MAX
                || page.index >= selected.page_count
                || page.width == 0
                || page.height == 0
                || page.width > SIDE_MAX
                || page.height > SIDE_MAX
                || u64::from(page.width) * u64::from(page.height) > u64::from(self.pixels)
                || page.ifd_offset < 8
                || page.ifd_offset >= self.source_bytes
                || selected.source_sha256 != self.source_sha256
            {
                return Err(invalid("TIFF browser selected inventory changed"));
            }
        }
        Ok(())
    }
    pub fn operation(&self) -> u8 {
        if self.selection.is_some() { 2 } else { 1 }
    }
    pub fn page_index(&self) -> u16 {
        self.selection.map_or(NONE, |selected| selected.page.index)
    }
    pub fn encode(&self) -> [u8; REQUEST_HEADER] {
        let mut out = [0; REQUEST_HEADER];
        out[..8].copy_from_slice(b"PCTIF001");
        out[8..10].copy_from_slice(&VERSION.to_be_bytes());
        out[10] = self.operation();
        out[12..14].copy_from_slice(&self.page_index().to_be_bytes());
        out[16..24].copy_from_slice(&self.source_bytes.to_be_bytes());
        write_owner(&mut out[24..65], self.owner);
        out[65..81].copy_from_slice(self.nonce.as_bytes());
        out[81..113].copy_from_slice(&self.source_sha256);
        out[145..149].copy_from_slice(&self.pixels.to_be_bytes());
        if let Some(selected) = self.selection {
            out[113..145].copy_from_slice(&selected.source_sha256);
            out[149..151].copy_from_slice(&selected.page_count.to_be_bytes());
            out[151..155].copy_from_slice(&selected.page.width.to_be_bytes());
            out[155..159].copy_from_slice(&selected.page.height.to_be_bytes());
            out[159..167].copy_from_slice(&selected.page.ifd_offset.to_be_bytes());
        }
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REQUEST_HEADER
            || &bytes[..8] != b"PCTIF001"
            || word16(bytes, 8) != VERSION
            || !matches!(bytes[10], 1 | 2)
            || bytes[11] != 0
            || bytes[14..16] != [0; 2]
            || bytes[167..] != [0; 9]
        {
            return Err(invalid("TIFF browser request framing"));
        }
        let index = word16(bytes, 12);
        let selection = if bytes[10] == 2 {
            Some(Selection {
                page: Page {
                    index,
                    width: word32(bytes, 151),
                    height: word32(bytes, 155),
                    ifd_offset: word64(bytes, 159),
                    supported: true,
                },
                page_count: word16(bytes, 149),
                source_sha256: bytes[113..145].try_into().unwrap(),
            })
        } else {
            if index != NONE || bytes[113..145] != [0; 32] || bytes[149..167] != [0; 18] {
                return Err(invalid("TIFF inventory carries a selection"));
            }
            None
        };
        let value = Self {
            source_bytes: word64(bytes, 16),
            source_sha256: bytes[81..113].try_into().unwrap(),
            owner: read_owner(&bytes[24..65]),
            nonce: Uuid::from_slice(&bytes[65..81]).unwrap(),
            pixels: word32(bytes, 145),
            selection,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn verify_source(&self, source: &[u8]) -> Result<()> {
        self.validate()?;
        if source.len() as u64 != self.source_bytes || digest(source) != self.source_sha256 {
            return Err(invalid("TIFF whole-source digest changed"));
        }
        Ok(())
    }
}
pub fn encode_inventory(pages: &[Page]) -> Result<Vec<u8>> {
    if pages.is_empty() || pages.len() > PAGES_MAX as usize {
        return Err(invalid("TIFF inventory count"));
    }
    let mut out = vec![0; pages.len() * ROW_BYTES];
    for (index, (page, row)) in pages
        .iter()
        .zip(out.as_chunks_mut::<ROW_BYTES>().0)
        .enumerate()
    {
        if page.index as usize != index {
            return Err(invalid("TIFF inventory order"));
        }
        row[..2].copy_from_slice(&page.index.to_be_bytes());
        row[2] = u8::from(page.supported);
        row[4..8].copy_from_slice(&page.width.to_be_bytes());
        row[8..12].copy_from_slice(&page.height.to_be_bytes());
        row[12..20].copy_from_slice(&page.ifd_offset.to_be_bytes());
    }
    Ok(out)
}
pub fn decode_inventory(bytes: &[u8], count: u16, source_bytes: u64) -> Result<Vec<Page>> {
    if count == 0 || count > PAGES_MAX || bytes.len() != count as usize * ROW_BYTES {
        return Err(invalid("TIFF inventory extent"));
    }
    let mut pages = Vec::with_capacity(count as usize);
    for (index, row) in bytes.as_chunks::<ROW_BYTES>().0.iter().enumerate() {
        let page = Page {
            index: word16(row, 0),
            width: word32(row, 4),
            height: word32(row, 8),
            ifd_offset: word64(row, 12),
            supported: row[2] == 1,
        };
        if page.index as usize != index
            || row[2] > 1
            || row[3] != 0
            || row[20..] != [0; 4]
            || page.ifd_offset < 8
            || page.ifd_offset >= source_bytes
            || (page.supported
                && (page.width == 0
                    || page.height == 0
                    || page.width > SIDE_MAX
                    || page.height > SIDE_MAX))
        {
            return Err(invalid("TIFF inventory row"));
        }
        pages.push(page);
    }
    Ok(pages)
}
#[derive(Debug)]
pub enum Frame<'a> {
    Success {
        page_count: u16,
        width: u32,
        height: u32,
        report: &'a [u8],
        payload: &'a [u8],
    },
    Refused(Error),
}
pub fn response(
    header: &Header,
    version: &str,
    page_count: u16,
    width: u32,
    height: u32,
    report: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>> {
    frame(
        header, version, 0, page_count, width, height, report, payload,
    )
}
pub fn refusal(header: &Header, version: &str, error: &Error) -> Result<Vec<u8>> {
    let status = match error.code {
        ErrorCode::CapabilityUnsupported => 2,
        ErrorCode::ResourceLimit => 3,
        ErrorCode::OperationCancelled => 4,
        ErrorCode::ConflictStaleBase => 5,
        _ => 1,
    };
    frame(header, version, status, 0, 0, 0, &[], &[])
}
#[allow(clippy::too_many_arguments)]
fn frame(
    header: &Header,
    version: &str,
    status: u8,
    count: u16,
    width: u32,
    height: u32,
    report: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>> {
    if version.is_empty()
        || version.len() > 64
        || !version.is_ascii()
        || report.len() > REPORT_BYTES
        || payload.len() > PIXELS_MAX as usize * 4
    {
        return Err(invalid("TIFF response bounds"));
    }
    let mut out = vec![0; RESPONSE_HEADER];
    out[..8].copy_from_slice(b"PCTIR001");
    out[8..10].copy_from_slice(&VERSION.to_be_bytes());
    out[10] = header.operation();
    out[11] = status;
    out[12..14].copy_from_slice(&header.page_index().to_be_bytes());
    out[14..16].copy_from_slice(&count.to_be_bytes());
    out[16..20].copy_from_slice(&width.to_be_bytes());
    out[20..24].copy_from_slice(&height.to_be_bytes());
    out[24..32].copy_from_slice(&(payload.len() as u64).to_be_bytes());
    out[32..36].copy_from_slice(&(report.len() as u32).to_be_bytes());
    out[36..38].copy_from_slice(&(version.len() as u16).to_be_bytes());
    write_owner(&mut out[40..81], header.owner);
    out[81..97].copy_from_slice(header.nonce.as_bytes());
    out[97..129].copy_from_slice(&header.source_sha256);
    out[129..161].copy_from_slice(&digest(payload));
    out[161..193].copy_from_slice(&digest(report));
    out.extend_from_slice(version.as_bytes());
    out.extend_from_slice(report);
    out.extend_from_slice(payload);
    decode_response(&out, header, header.owner, version)?;
    Ok(out)
}
pub fn decode_response<'a>(
    bytes: &'a [u8],
    header: &Header,
    owner: Owner,
    version: &str,
) -> Result<Frame<'a>> {
    header.validate()?;
    if bytes.len() < RESPONSE_HEADER
        || bytes.len() > OUTPUT_MAX
        || &bytes[..8] != b"PCTIR001"
        || word16(bytes, 8) != VERSION
        || bytes[10] != header.operation()
        || bytes[11] > 5
        || word16(bytes, 12) != header.page_index()
        || bytes[38..40] != [0; 2]
        || bytes[193..200] != [0; 7]
        || read_owner(&bytes[40..81]) != owner
        || owner != header.owner
        || &bytes[81..97] != header.nonce.as_bytes()
        || bytes[97..129] != header.source_sha256
    {
        return Err(invalid("TIFF response framing/owner/source"));
    }
    let version_len = word16(bytes, 36) as usize;
    let report_len = word32(bytes, 32) as usize;
    let payload_len =
        usize::try_from(word64(bytes, 24)).map_err(|_| invalid("TIFF payload extent"))?;
    let extent = RESPONSE_HEADER
        .checked_add(version_len)
        .and_then(|n| n.checked_add(report_len))
        .and_then(|n| n.checked_add(payload_len))
        .ok_or_else(|| invalid("TIFF response overflow"))?;
    if version_len == 0
        || version_len > 64
        || report_len > REPORT_BYTES
        || extent != bytes.len()
        || &bytes[RESPONSE_HEADER..RESPONSE_HEADER + version_len] != version.as_bytes()
    {
        return Err(invalid("TIFF response extent/version"));
    }
    let report = &bytes[RESPONSE_HEADER + version_len..RESPONSE_HEADER + version_len + report_len];
    let payload = &bytes[RESPONSE_HEADER + version_len + report_len..];
    if digest(report) != bytes[161..193] || digest(payload) != bytes[129..161] {
        return Err(invalid("TIFF response payload/report digest"));
    }
    let count = word16(bytes, 14);
    let width = word32(bytes, 16);
    let height = word32(bytes, 20);
    if bytes[11] != 0 {
        if count != 0 || width != 0 || height != 0 || !report.is_empty() || !payload.is_empty() {
            return Err(invalid("TIFF refusal carries image data"));
        }
        let code = match bytes[11] {
            2 => ErrorCode::CapabilityUnsupported,
            3 => ErrorCode::ResourceLimit,
            4 => ErrorCode::OperationCancelled,
            5 => ErrorCode::ConflictStaleBase,
            _ => ErrorCode::CodecFailed,
        };
        return Ok(Frame::Refused(Error::new(
            code,
            "Isolated TIFF page import refused",
        )));
    }
    if let Some(selected) = header.selection {
        if count != selected.page_count
            || width != selected.page.width
            || height != selected.page.height
            || report.len() != REPORT_BYTES
            || payload.len() as u64 != u64::from(width) * u64::from(height) * 4
        {
            return Err(invalid("TIFF selected response differs from inventory"));
        }
    } else {
        if width != 0 || height != 0 || !report.is_empty() {
            return Err(invalid("TIFF inventory carries image data"));
        }
        decode_inventory(payload, count, header.source_bytes)?;
    }
    Ok(Frame::Success {
        page_count: count,
        width,
        height,
        report,
        payload,
    })
}
fn write_owner(bytes: &mut [u8], owner: Owner) {
    bytes[..16].copy_from_slice(owner.session.as_bytes());
    bytes[16..32].copy_from_slice(owner.state.as_bytes());
    bytes[32..40].copy_from_slice(&owner.epoch.to_be_bytes());
    bytes[40] = owner.frontend;
}
fn read_owner(bytes: &[u8]) -> Owner {
    Owner {
        session: Uuid::from_slice(&bytes[..16]).unwrap(),
        state: Uuid::from_slice(&bytes[16..32]).unwrap(),
        epoch: word64(bytes, 32),
        frontend: bytes[40],
    }
}
fn word16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn word32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn word64(bytes: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(bytes[at..at + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests;
