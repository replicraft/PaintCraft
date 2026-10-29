//! Fixed-width report. Policy, precision, geometry and statistics are included
//! in its canonical bytes; callers can bind assessment consent to their digest.
use super::*;
pub const REPORT_BYTES: usize = 96;
pub const POLICY: &str = "jxl-integer8-16-to-srgb8-nearest257-v1";
const MAGIC: &[u8; 8] = b"PCJXLI01";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub source_depth: u8,
    pub width: u32,
    pub height: u32,
    pub source_sha256: [u8; 32],
    pub output_sha256: [u8; 32],
    pub changed_rgb_components: u32,
    pub changed_alpha: u32,
}
impl Report {
    pub fn policy(&self) -> &'static str {
        POLICY
    }
    pub fn losses(&self) -> &'static [&'static str] {
        if self.source_depth == 16 {
            &[crate::jxl::DEPTH16_LOSS]
        } else {
            &[]
        }
    }
    fn validate(&self) -> Result<()> {
        let pixels = u64::from(self.width) * u64::from(self.height);
        if !matches!(self.source_depth, 8 | 16)
            || self.width == 0
            || self.height == 0
            || self.width > 65536
            || self.height > 65536
            || pixels > PIXELS_MAX
            || u64::from(self.changed_rgb_components) > pixels * 3
            || u64::from(self.changed_alpha) > pixels
            || (self.source_depth == 8
                && (self.changed_rgb_components != 0 || self.changed_alpha != 0))
        {
            return Err(invalid("Integer JXL report bounds/policy"));
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<[u8; REPORT_BYTES]> {
        self.validate()?;
        let mut out = [0; REPORT_BYTES];
        out[..8].copy_from_slice(MAGIC);
        out[8..10].copy_from_slice(&1u16.to_be_bytes());
        out[10] = self.source_depth;
        out[12..16].copy_from_slice(&self.width.to_be_bytes());
        out[16..20].copy_from_slice(&self.height.to_be_bytes());
        out[20..52].copy_from_slice(&self.source_sha256);
        out[52..84].copy_from_slice(&self.output_sha256);
        out[84..88].copy_from_slice(&self.changed_rgb_components.to_be_bytes());
        out[88..92].copy_from_slice(&self.changed_alpha.to_be_bytes());
        out[92..94].copy_from_slice(&u16::from(self.source_depth == 16).to_be_bytes());
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REPORT_BYTES
            || &bytes[..8] != MAGIC
            || bytes[8..10] != 1u16.to_be_bytes()
            || bytes[11] != 0
            || bytes[94..96] != [0; 2]
            || bytes[92..94] != u16::from(bytes[10] == 16).to_be_bytes()
        {
            return Err(invalid("Integer JXL report framing/policy"));
        }
        let word = |at| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
        let value = Self {
            source_depth: bytes[10],
            width: word(12),
            height: word(16),
            source_sha256: bytes[20..52].try_into().unwrap(),
            output_sha256: bytes[52..84].try_into().unwrap(),
            changed_rgb_components: word(84),
            changed_alpha: word(88),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate_payload(
        &self,
        request: Header,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<()> {
        self.validate()?;
        if request.codec != Codec::JxlIntegerRgba8
            || self.source_sha256 != request.sha256
            || (self.width, self.height) != (width, height)
            || u64::from(width) * u64::from(height) > u64::from(request.pixels)
            || rgba.len() as u64 != u64::from(width) * u64::from(height) * 4
            || self.output_sha256 != <[u8; 32]>::from(Sha256::digest(rgba))
        {
            return Err(invalid("Integer JXL source/output report binding"));
        }
        Ok(())
    }
}
