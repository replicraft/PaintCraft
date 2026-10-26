use super::*;

#[derive(Debug)]
pub struct EncodedFrame<'a> {
    bytes: &'a [u8],
    sha256: [u8; 32],
    heap_peak: u64,
    inspection: crate::jxr::Inspection,
}
impl<'a> EncodedFrame<'a> {
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    /// Requested Rust heap observation, never a physical RSS measurement.
    pub fn heap_peak(&self) -> u64 {
        self.heap_peak
    }
    pub fn inspection(&self) -> crate::jxr::Inspection {
        self.inspection
    }
    pub fn profile(&self) -> &'static str {
        PROFILE
    }
    pub fn density_policy(&self) -> &'static str {
        DENSITY_POLICY
    }
}
fn fresh(
    bytes: &[u8],
    layout: Layout,
    check: &dyn Fn() -> Result<()>,
) -> Result<crate::jxr::Inspection> {
    if bytes.is_empty() || bytes.len() as u64 > layout.encoded_cap {
        return Err(resource("JXR encode output cap"));
    }
    let inspection = crate::jxr::inspect_fresh_rgb8_checked(bytes, PIXELS_MAX, check)?;
    if (inspection.width, inspection.height) != (layout.width, layout.height)
        || inspection.container_version != 1
        || inspection.resolution_x_bits != Some(0)
        || inspection.resolution_y_bits != Some(0)
    {
        return Err(invalid(
            "JXR encode output fixed profile/density/geometry mismatch",
        ));
    }
    Ok(inspection)
}
fn header(
    request: Header,
    status: u8,
    length: u64,
    output_hash: [u8; 32],
    detail: usize,
    peak: u64,
) -> Result<[u8; RESPONSE_HEADER]> {
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    if version.is_empty()
        || version.len() > VERSION_BYTES_MAX
        || detail > DETAIL_BYTES_MAX
        || peak > HEAP_BYTES
    {
        return Err(resource("JXR encode response header bounds"));
    }
    let mut out = [0; RESPONSE_HEADER];
    out[..8].copy_from_slice(RESPONSE_MAGIC);
    out[8..10].copy_from_slice(&VERSION.to_be_bytes());
    out[10] = status;
    out[11] = PROFILE_TAG;
    if status == 0 {
        out[12..16].copy_from_slice(&request.layout().width.to_be_bytes());
        out[16..20].copy_from_slice(&request.layout().height.to_be_bytes());
    }
    out[20..28].copy_from_slice(&length.to_be_bytes());
    out[28..36].copy_from_slice(&peak.to_be_bytes());
    put_owner(&mut out[36..77], request.owner());
    out[77..93].copy_from_slice(request.nonce().as_bytes());
    out[93..125].copy_from_slice(&request.request_sha256());
    out[125..157].copy_from_slice(&request.rgb_sha256());
    out[157..189].copy_from_slice(&output_hash);
    out[189..191].copy_from_slice(&(version.len() as u16).to_be_bytes());
    out[191..193].copy_from_slice(&(detail as u16).to_be_bytes());
    Ok(out)
}
/// Validate structure/provenance, not an independent proof of encoded pixels.
/// Output and the caller-owned sink must be admitted before writing this frame.
pub fn write_success(
    request: Header,
    encoded: &[u8],
    heap_peak: u64,
    writer: &mut impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    fresh(encoded, request.layout(), check)?;
    let output_hash = hash(encoded, check)?;
    let header = header(request, 0, encoded.len() as u64, output_hash, 0, heap_peak)?;
    write_checked(writer, &header, check)?;
    write_checked(writer, env!("CARGO_PKG_VERSION").as_bytes(), check)?;
    write_checked(writer, encoded, check)
}
pub fn write_failure(
    request: Header,
    error: &Error,
    heap_peak: u64,
    writer: &mut impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let status = match error.code {
        ErrorCode::CapabilityUnsupported => 1,
        ErrorCode::ResourceLimit => 3,
        ErrorCode::OperationCancelled => 4,
        _ => 2,
    };
    let mut end = error.message.len().min(DETAIL_BYTES_MAX);
    while !error.message.is_char_boundary(end) {
        end -= 1;
    }
    let detail = &error.message.as_bytes()[..end];
    let header = header(request, status, 0, [0; 32], detail.len(), heap_peak)?;
    write_checked(writer, &header, check)?;
    write_checked(writer, env!("CARGO_PKG_VERSION").as_bytes(), check)?;
    write_checked(writer, detail, check)
}
/// A borrowed private encoded-file view. Caller repeats source/owner/publication
/// guards; a matching hash is not an independent entropy/pixel comparison.
pub fn decode_response<'a>(
    bytes: &'a [u8],
    request: Header,
    current: Owner,
    check: &dyn Fn() -> Result<()>,
) -> Result<EncodedFrame<'a>> {
    check()?;
    if current != request.owner() {
        return Err(stale());
    }
    if bytes.len() < RESPONSE_HEADER || bytes.len() as u64 > request.layout().response_limit()? {
        return Err(resource("JXR encode response extent"));
    }
    if bytes[..8] != *RESPONSE_MAGIC
        || bytes[8..10] != VERSION.to_be_bytes()
        || bytes[10] > 4
        || bytes[11] != PROFILE_TAG
    {
        return Err(unsupported("JXR encode response protocol/fixed profile"));
    }
    if bytes[193..RESPONSE_HEADER].iter().any(|n| *n != 0) {
        return Err(invalid("JXR encode response reserved fields"));
    }
    let owner = get_owner(&bytes[36..77]);
    if owner != request.owner()
        || bytes[77..93] != request.nonce().as_bytes()[..]
        || bytes[93..125] != request.request_sha256()
        || bytes[125..157] != request.rgb_sha256()
    {
        return Err(stale());
    }
    let u32_at = |n| u32::from_be_bytes(bytes[n..n + 4].try_into().unwrap());
    let u64_at = |n| u64::from_be_bytes(bytes[n..n + 8].try_into().unwrap());
    let status = bytes[10];
    let length = u64_at(20);
    let peak = u64_at(28);
    let version = usize::from(u16::from_be_bytes(bytes[189..191].try_into().unwrap()));
    let detail = usize::from(u16::from_be_bytes(bytes[191..193].try_into().unwrap()));
    if peak > HEAP_BYTES
        || version == 0
        || version > VERSION_BYTES_MAX
        || detail > DETAIL_BYTES_MAX
        || length > request.layout().encoded_cap
    {
        return Err(resource("JXR encode response declarations exceed bounds"));
    }
    let prefix = RESPONSE_HEADER
        .checked_add(version)
        .and_then(|n| n.checked_add(detail))
        .ok_or_else(|| resource("JXR encode response prefix overflow"))?;
    let length =
        usize::try_from(length).map_err(|_| resource("JXR encode response address extent"))?;
    if prefix.checked_add(length) != Some(bytes.len()) {
        return Err(invalid("JXR encode response truncated/extra payload"));
    }
    if &bytes[RESPONSE_HEADER..RESPONSE_HEADER + version] != env!("CARGO_PKG_VERSION").as_bytes() {
        return Err(unsupported("JXR encode response application version"));
    }
    let output_hash: [u8; 32] = bytes[157..189].try_into().unwrap();
    if status != 0 {
        if length != 0 || u32_at(12) != 0 || u32_at(16) != 0 || output_hash != [0; 32] {
            return Err(invalid("JXR encode failure carries encoded output"));
        }
        let message = std::str::from_utf8(&bytes[RESPONSE_HEADER + version..prefix])
            .map_err(|_| invalid("JXR encode failure is not UTF-8"))?;
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
    if detail != 0 || u32_at(12) != request.layout().width || u32_at(16) != request.layout().height
    {
        return Err(invalid("JXR encode success geometry/detail"));
    }
    let encoded = &bytes[prefix..];
    if hash(encoded, check)? != output_hash {
        return Err(invalid("JXR encode output digest mismatch"));
    }
    let inspection = fresh(encoded, request.layout(), check)?;
    check()?;
    Ok(EncodedFrame {
        bytes: encoded,
        sha256: output_hash,
        heap_peak: peak,
        inspection,
    })
}
