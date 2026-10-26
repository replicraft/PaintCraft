//! Shared isolated codec-worker envelopes. Parsing never publishes state.
#[cfg(feature = "heif-codec")]
mod decode;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(feature = "heif-codec")]
pub use decode::execute;
use paintcraft_core::types::{Error, ErrorCode, Result};
pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const PIXELS_MAX: u64 = 4_194_304;
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
pub const HEAP_BYTES: u64 = 256 << 20;
pub const RESOURCE_EXIT: i32 = 75;
pub const REQUEST_HEADER: usize = 112;
pub const RESPONSE_HEADER: usize = 136;
pub const OUTPUT_BYTES_MAX: usize = PIXELS_MAX as usize * 4 + RESPONSE_HEADER + 4096 + 64;
pub const REQUEST_MAGIC: &[u8; 8] = b"PCDEC002";
pub const PROTOCOL_VERSION: u16 = 2;
pub const NATIVE_PROFILE: &str =
    "internal-heif-gray8-jxl-rgba8-jxr-rgb8-private-source1-parent-lease1";
pub const BUILD_RECORD_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V3\0";
/// Expected native build bytes, provided to the OS verifier at composition.
pub fn native_build_record(version: &str) -> Result<Vec<u8>> {
    if version.is_empty() || version.len() > 64 || !version.is_ascii() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Invalid codec application version",
        ));
    }
    let mut bytes = BUILD_RECORD_MARKER.to_vec();
    bytes.extend(version.as_bytes());
    bytes.push(0);
    bytes.extend(REQUEST_MAGIC);
    bytes.push(0);
    bytes.extend(format!("HEAP{HEAP_BYTES}").as_bytes());
    bytes.extend(b"\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0END_CODEC_WORKER_BUILD\0");
    Ok(bytes)
}
/// Reserve the fixed native child ceiling and parent buffers before reading the
/// pre-admitted source. Verification/source IO slack is added by the host.
pub fn reservation_bytes_for_input(length: u64) -> Result<u64> {
    if length == 0 || length > INPUT_BYTES_MAX as u64 {
        return Err(resource("worker source allowance"));
    }
    Ok(HEAP_BYTES + length + OUTPUT_BYTES_MAX as u64 + 65536)
}

const RESPONSE_MAGIC: &[u8; 8] = b"PCPIX002";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Codec {
    HeifGray8 = 1,
    JxlRgba8 = 2,
    JxrRgb8 = 3,
}
impl Codec {
    fn side_limit(self) -> u32 {
        match self {
            Self::HeifGray8 => 16384,
            Self::JxlRgba8 | Self::JxrRgb8 => 65536,
        }
    }
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::HeifGray8),
            2 => Ok(Self::JxlRgba8),
            3 if !cfg!(target_arch = "wasm32") => Ok(Self::JxrRgb8),
            _ => Err(unsupported("worker codec request is not supported")),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub session: Uuid,
    pub state: Uuid,
    pub epoch: u64,
    pub frontend: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub codec: Codec,
    pub nonce: Uuid,
    pub owner: Owner,
    pub sha256: [u8; 32],
    pub length: u64,
    pub pixels: u32,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub header: Header,
    pub source: Arc<Vec<u8>>,
}
impl Request {
    pub fn capture(source: Arc<Vec<u8>>, owner: Owner, pixels: u64) -> Result<Self> {
        Self::capture_for(Codec::HeifGray8, source, owner, pixels)
    }
    pub fn capture_for(
        codec: Codec,
        source: Arc<Vec<u8>>,
        owner: Owner,
        pixels: u64,
    ) -> Result<Self> {
        if cfg!(target_arch = "wasm32") && codec == Codec::JxrRgb8 {
            return Err(unsupported("JXR worker is native-only"));
        }
        if owner.frontend != 3 {
            return Err(unsupported("native codec worker is Studio-only"));
        }
        if source.is_empty() || source.len() > INPUT_BYTES_MAX || pixels == 0 || pixels > PIXELS_MAX
        {
            return Err(resource("native codec worker request allowance"));
        }
        let sha256 = Sha256::digest(source.as_slice()).into();
        Ok(Self {
            header: Header {
                codec,
                nonce: Uuid::new_v4(),
                owner,
                sha256,
                length: source.len() as u64,
                pixels: pixels as u32,
            },
            source,
        })
    }
    pub fn reservation_bytes(&self) -> u64 {
        HEAP_BYTES + self.header.length + OUTPUT_BYTES_MAX as u64 + 65536
    }
    pub fn segments(&self) -> Vec<Arc<Vec<u8>>> {
        vec![Arc::new(self.header.encode()), self.source.clone()]
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .pos
            .checked_add(N)
            .ok_or_else(|| invalid("worker field overflow"))?;
        let out = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated worker header"))?
            .try_into()
            .unwrap();
        self.pos = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take()?))
    }
    fn uuid(&mut self) -> Result<Uuid> {
        Ok(Uuid::from_bytes(self.take()?))
    }
    fn owner(&mut self) -> Result<Owner> {
        Ok(Owner {
            session: self.uuid()?,
            state: self.uuid()?,
            epoch: self.u64()?,
            frontend: self.u8()?,
        })
    }
}
fn owner_bytes(out: &mut Vec<u8>, owner: Owner) {
    out.extend(owner.session.as_bytes());
    out.extend(owner.state.as_bytes());
    out.extend(owner.epoch.to_be_bytes());
    out.push(owner.frontend);
}
impl Header {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(REQUEST_HEADER);
        out.extend(REQUEST_MAGIC);
        out.extend(PROTOCOL_VERSION.to_be_bytes());
        out.push(self.codec as u8);
        out.extend(self.pixels.to_be_bytes());
        out.extend(self.length.to_be_bytes());
        owner_bytes(&mut out, self.owner);
        out.extend(self.nonce.as_bytes());
        out.extend(self.sha256);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REQUEST_HEADER {
            return Err(invalid("worker request header length"));
        }
        let mut r = Reader { bytes, pos: 0 };
        if r.take::<8>()? != *REQUEST_MAGIC || r.u16()? != PROTOCOL_VERSION {
            return Err(unsupported("worker request protocol"));
        }
        let codec = Codec::decode(r.u8()?)?;
        let pixels = r.u32()?;
        let length = r.u64()?;
        let owner = r.owner()?;
        let nonce = r.uuid()?;
        let sha256 = r.take()?;
        if cfg!(target_arch = "wasm32") && codec == Codec::JxrRgb8 {
            return Err(unsupported("JXR worker is native-only"));
        }
        if owner.frontend != 3 {
            return Err(unsupported("native codec worker is Studio-only"));
        }
        if pixels == 0
            || u64::from(pixels) > PIXELS_MAX
            || length == 0
            || length > INPUT_BYTES_MAX as u64
        {
            return Err(resource("worker input declaration exceeds allowance"));
        }
        Ok(Self {
            codec,
            nonce,
            owner,
            sha256,
            length,
            pixels,
        })
    }
    pub fn verify_source(&self, bytes: &[u8]) -> Result<()> {
        if bytes.len() as u64 != self.length
            || <[u8; 32]>::from(Sha256::digest(bytes)) != self.sha256
        {
            return Err(invalid("worker source digest/length mismatch"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeEncoding {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pixels<'a> {
    NativeGray8 {
        gray: &'a [u8],
        alpha: Option<&'a [u8]>,
        encoding: NativeEncoding,
    },
    /// Integer IEC sRGB with straight alpha, four bytes per pixel.
    SrgbRgba8(&'a [u8]),
}
#[derive(Debug)]
pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels<'a>,
    /// Requested-heap snapshot during response preparation, never RSS.
    pub heap_peak: u64,
}
#[derive(Debug)]
pub struct Success<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels<'a>,
}
#[allow(clippy::too_many_arguments)] // Fixed-width wire fields, not caller configuration.
fn response_header(
    request: Header,
    status: u8,
    width: u32,
    height: u32,
    encoding: NativeEncoding,
    y: usize,
    a: usize,
    detail: usize,
    version: usize,
    peak: u64,
) -> Result<Vec<u8>> {
    if peak > HEAP_BYTES || detail > 4096 || version > 64 {
        return Err(resource("worker response metadata allowance"));
    }
    let mut out = Vec::with_capacity(RESPONSE_HEADER);
    out.extend(RESPONSE_MAGIC);
    out.extend(PROTOCOL_VERSION.to_be_bytes());
    out.push(status);
    out.push(request.codec as u8);
    out.extend(width.to_be_bytes());
    out.extend(height.to_be_bytes());
    out.push(u8::from(a != 0));
    out.extend(encoding.primaries.to_be_bytes());
    out.extend(encoding.transfer.to_be_bytes());
    out.extend(encoding.matrix.to_be_bytes());
    out.push(u8::from(encoding.full_range));
    out.extend((y as u32).to_be_bytes());
    out.extend((a as u32).to_be_bytes());
    out.extend((detail as u16).to_be_bytes());
    out.push(version as u8);
    out.extend(peak.to_be_bytes());
    owner_bytes(&mut out, request.owner);
    out.extend(request.nonce.as_bytes());
    out.extend(request.sha256);
    Ok(out)
}
/// The parent knows this exact fixed allocator ceiling before launching. No
/// smaller per-request heap number is represented or claimed to be enforced.
pub fn success(request: Header, result: Success<'_>, peak: u64) -> Result<Vec<u8>> {
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    let (primary, alpha, encoding, channels) = match (request.codec, result.pixels) {
        (
            Codec::HeifGray8,
            Pixels::NativeGray8 {
                gray,
                alpha,
                encoding,
            },
        ) => (gray, alpha, encoding, 1),
        (Codec::JxlRgba8 | Codec::JxrRgb8, Pixels::SrgbRgba8(rgba)) => (
            rgba,
            None,
            NativeEncoding {
                primaries: 1,
                transfer: 13,
                matrix: 0,
                full_range: true,
            },
            4,
        ),
        _ => return Err(invalid("worker response codec/sample model mismatch")),
    };
    let y = primary.len();
    let a = alpha.map_or(0, <[u8]>::len);
    let pixels = u64::from(result.width) * u64::from(result.height);
    if result.width == 0
        || result.height == 0
        || result.width > request.codec.side_limit()
        || result.height > request.codec.side_limit()
        || pixels > PIXELS_MAX
        || pixels > u64::from(request.pixels)
        || y as u64 != pixels * channels
        || a != 0 && a != y
    {
        return Err(invalid("worker success dimensions/plane lengths"));
    }
    let mut out = response_header(
        request,
        0,
        result.width,
        result.height,
        encoding,
        y,
        a,
        0,
        version.len(),
        peak,
    )?;
    out.try_reserve(version.len() + y + a)
        .map_err(|_| resource("worker response allocation failed"))?;
    out.extend(version);
    out.extend(primary);
    if let Some(alpha) = alpha {
        out.extend(alpha);
    }
    Ok(out)
}
pub fn error(request: Header, error: &Error, peak: u64) -> Result<Vec<u8>> {
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    let detail = error.message.chars().take(1024).collect::<String>();
    let status = match error.code {
        ErrorCode::CapabilityUnsupported => 1,
        ErrorCode::ResourceLimit => 3,
        ErrorCode::OperationCancelled => 4,
        _ => 2,
    };
    let mut out = response_header(
        request,
        status,
        0,
        0,
        NativeEncoding {
            primaries: 0,
            transfer: 0,
            matrix: 0,
            full_range: false,
        },
        0,
        0,
        detail.len(),
        version.len(),
        peak,
    )?;
    out.extend(version);
    out.extend(detail.as_bytes());
    Ok(out)
}
/// Owner is sampled again by the caller on its publication thread. A match here
/// releases only a PRIVATE native plane view, never a document or saved state.
pub fn decode<'a>(bytes: &'a [u8], request: &Request, current: Owner) -> Result<Frame<'a>> {
    if current != request.header.owner {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "worker document/frontend owner changed",
        ));
    }
    if bytes.len() < RESPONSE_HEADER || bytes.len() > OUTPUT_BYTES_MAX {
        return Err(resource("worker output size allowance"));
    }
    let mut r = Reader { bytes, pos: 0 };
    if r.take::<8>()? != *RESPONSE_MAGIC || r.u16()? != PROTOCOL_VERSION || r.u8()? > 4 {
        return Err(invalid("worker response protocol"));
    }
    if Codec::decode(r.u8()?)? != request.header.codec {
        return Err(invalid("worker response protocol"));
    }
    let status = bytes[10];
    let width = r.u32()?;
    let height = r.u32()?;
    let alpha_flag = r.u8()?;
    let encoding = NativeEncoding {
        primaries: r.u16()?,
        transfer: r.u16()?,
        matrix: r.u16()?,
        full_range: match r.u8()? {
            0 => false,
            1 => true,
            _ => return Err(invalid("worker range flag")),
        },
    };
    let y = r.u32()? as usize;
    let a = r.u32()? as usize;
    let detail = usize::from(r.u16()?);
    let version = usize::from(r.u8()?);
    let peak = r.u64()?;
    let owner = r.owner()?;
    let nonce = r.uuid()?;
    let sha256 = r.take::<32>()?;
    if owner != request.header.owner
        || nonce != request.header.nonce
        || sha256 != request.header.sha256
    {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "worker source/job identity changed",
        ));
    }
    if r.pos != RESPONSE_HEADER
        || peak > HEAP_BYTES
        || detail > 4096
        || version > 64
        || alpha_flag > 1
    {
        return Err(invalid("worker response header bounds"));
    }
    let total = RESPONSE_HEADER
        .checked_add(version)
        .and_then(|n| n.checked_add(detail))
        .and_then(|n| n.checked_add(y))
        .and_then(|n| n.checked_add(a))
        .ok_or_else(|| resource("worker output length overflow"))?;
    if total != bytes.len() {
        return Err(invalid("worker response has a truncated/extra payload"));
    }
    let body = &bytes[RESPONSE_HEADER..];
    if body.get(..version) != Some(env!("CARGO_PKG_VERSION").as_bytes()) {
        return Err(unsupported("worker application version mismatch"));
    }
    if status != 0 {
        if y != 0 || a != 0 || width != 0 || height != 0 {
            return Err(invalid("worker failure carries pixel data"));
        }
        let message = std::str::from_utf8(&body[version..version + detail])
            .map_err(|_| invalid("worker error text is not UTF-8"))?;
        return Err(Error::new(
            match status {
                1 => ErrorCode::CapabilityUnsupported,
                3 => ErrorCode::ResourceLimit,
                4 => ErrorCode::OperationCancelled,
                _ => ErrorCode::CodecFailed,
            },
            message,
        ));
    }
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > request.header.codec.side_limit()
        || height > request.header.codec.side_limit()
        || pixels > PIXELS_MAX
        || pixels > u64::from(request.header.pixels)
        || detail != 0
    {
        return Err(invalid("worker output dimensions/detail"));
    }
    let payload = &body[version..];
    let pixels = match request.header.codec {
        Codec::HeifGray8 => {
            if y as u64 != pixels
                || a != if alpha_flag == 1 { y } else { 0 }
                || !encoding.full_range
                || !matches!(encoding.primaries, 1 | 2)
                || !matches!(encoding.transfer, 2 | 13)
                || !matches!(encoding.matrix, 0..=2)
            {
                return Err(invalid("worker native-gray profile/plane declarations"));
            }
            Pixels::NativeGray8 {
                gray: &payload[..y],
                alpha: (a != 0).then_some(&payload[y..]),
                encoding,
            }
        }
        Codec::JxlRgba8 | Codec::JxrRgb8 => {
            if y as u64 != pixels * 4
                || a != 0
                || alpha_flag != 0
                || encoding
                    != (NativeEncoding {
                        primaries: 1,
                        transfer: 13,
                        matrix: 0,
                        full_range: true,
                    })
            {
                return Err(invalid("worker sRGB RGBA8 profile/plane declarations"));
            }
            Pixels::SrgbRgba8(payload)
        }
    };
    Ok(Frame {
        width,
        height,
        pixels,
        heap_peak: peak,
    })
}

#[cfg(test)]
mod tests;
