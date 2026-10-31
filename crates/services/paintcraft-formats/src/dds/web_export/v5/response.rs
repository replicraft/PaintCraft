use super::*;
#[derive(Debug)]
pub enum Frame<'a> {
    Success {
        encoded: &'a [u8],
        preview: &'a [u8],
        preview_size: (u32, u32),
        losses: Vec<&'static str>,
        quantized_rgb_samples: u32,
    },
    /// Framing/binding has been validated, so this is a completed refusal.
    Refused(Error),
}
fn status(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::CapabilityUnsupported => 1,
        ErrorCode::ResourceLimit => 3,
        ErrorCode::InputSchema => 4,
        ErrorCode::OperationCancelled => 5,
        _ => 2,
    }
}
pub(super) fn refused(request: &Header, error: &Error, version: &str) -> Result<Vec<u8>> {
    let end = error
        .message
        .char_indices()
        .map(|(i, c)| i + c.len_utf8())
        .take_while(|n| *n <= 4096)
        .last()
        .unwrap_or(0);
    build(
        request,
        status(error.code),
        &[],
        &[],
        (0, 0),
        0,
        0,
        &error.message[..end],
        version,
    )
}
pub(super) fn success(
    request: &Header,
    artifact: &export::dds::PreparedDds,
    preview: &[u8],
    size: (u32, u32),
    version: &str,
) -> Result<Vec<u8>> {
    let mut mask = 0;
    for loss in artifact.losses() {
        mask |= 1u8
            << LOSSES
                .iter()
                .position(|known| *known == loss)
                .ok_or_else(|| failed("Unknown browser DDS loss"))?;
    }
    let quantized = u32::try_from(artifact.transfer().quantized_rgb_samples())
        .map_err(|_| failed("Browser DDS quantization count"))?;
    build(
        request,
        0,
        artifact.encoded().bytes(),
        preview,
        size,
        mask,
        quantized,
        "",
        version,
    )
}
#[allow(clippy::too_many_arguments)]
fn build(
    request: &Header,
    status: u8,
    encoded: &[u8],
    preview: &[u8],
    size: (u32, u32),
    losses: u8,
    quantized: u32,
    detail: &str,
    version: &str,
) -> Result<Vec<u8>> {
    if version.is_empty()
        || version.len() > 64
        || !version.is_ascii()
        || encoded.len() > ENCODED_BYTES_MAX
        || preview.len() > PREVIEW_BYTES_MAX
        || detail.len() > 4096
    {
        return Err(resource("Browser DDS response allowance"));
    }
    let mut bytes = Vec::new();
    let length = RESPONSE_HEADER + encoded.len() + preview.len() + detail.len();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| resource("Browser DDS response allocation"))?;
    bytes.resize(RESPONSE_HEADER, 0);
    bytes[..8].copy_from_slice(b"PCDDSR05");
    bytes[8..10].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    bytes[10] = status;
    bytes[12..16].copy_from_slice(&request.size.0.to_be_bytes());
    bytes[16..20].copy_from_slice(&request.size.1.to_be_bytes());
    bytes[20..24].copy_from_slice(&request.source_len.to_be_bytes());
    bytes[24..28].copy_from_slice(&(encoded.len() as u32).to_be_bytes());
    bytes[28..30].copy_from_slice(&(size.0 as u16).to_be_bytes());
    bytes[30..32].copy_from_slice(&(size.1 as u16).to_be_bytes());
    bytes[32..73].copy_from_slice(&request::owner_bytes(request.owner));
    bytes[73..89].copy_from_slice(request.nonce.as_bytes());
    bytes[89..121].copy_from_slice(&request.source_sha256);
    bytes[121..129].copy_from_slice(&request.settings);
    bytes[129..161].copy_from_slice(&Sha256::digest(encoded));
    bytes[161..193].copy_from_slice(&Sha256::digest(preview));
    bytes[193] = losses;
    bytes[194..198].copy_from_slice(&quantized.to_be_bytes());
    bytes[198] = version.len() as u8;
    bytes[199..199 + version.len()].copy_from_slice(version.as_bytes());
    bytes[263..265].copy_from_slice(&(detail.len() as u16).to_be_bytes());
    bytes[265] = request.source_flags;
    bytes[266] = request.bake_intent;
    bytes[288..320].copy_from_slice(&request.filter_digest);
    bytes[320..384].copy_from_slice(&request.metadata.encode());
    bytes[384..416].copy_from_slice(&request.masks.encode());
    bytes[416..480].copy_from_slice(&request.spatial.encode());
    bytes.extend_from_slice(encoded);
    bytes.extend_from_slice(preview);
    bytes.extend_from_slice(detail.as_bytes());
    Ok(bytes)
}
pub fn decode_response<'a>(
    bytes: &'a [u8],
    request: &Header,
    current: Owner,
    version: &str,
) -> Result<Frame<'a>> {
    request.validate()?;
    if current != request.owner {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "Browser DDS owner changed",
        ));
    }
    if bytes.len() < RESPONSE_HEADER
        || bytes.len() > RESPONSE_BYTES_MAX
        || &bytes[..8] != b"PCDDSR05"
        || bytes[8..10] != PROTOCOL_VERSION.to_be_bytes()
        || bytes[10] > 5
        || bytes[11] != 0
        || bytes[267..288].iter().any(|b| *b != 0)
    {
        return Err(failed("Browser DDS response framing/reserved fields"));
    }
    let width = u32::from_be_bytes(bytes[12..16].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let source = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    let encoded = u32::from_be_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let size = (
        u32::from(u16::from_be_bytes(bytes[28..30].try_into().unwrap())),
        u32::from(u16::from_be_bytes(bytes[30..32].try_into().unwrap())),
    );
    let preview = u64::from(size.0) * u64::from(size.1) * 4;
    let quantized = u32::from_be_bytes(bytes[194..198].try_into().unwrap());
    let version_len = usize::from(bytes[198]);
    let detail = usize::from(u16::from_be_bytes(bytes[263..265].try_into().unwrap()));
    if (width, height) != request.size
        || source != request.source_len
        || bytes[32..73] != request::owner_bytes(request.owner)
        || bytes[73..89] != *request.nonce.as_bytes()
        || bytes[89..121] != request.source_sha256
        || bytes[121..129] != request.settings
        || bytes[265] != request.source_flags
        || bytes[266] != request.bake_intent
        || bytes[288..320] != request.filter_digest
        || bytes[320..384] != request.metadata.encode()
        || bytes[384..416] != request.masks.encode()
        || bytes[416..480] != request.spatial.encode()
        || version_len == 0
        || version_len > 64
        || bytes[199..199 + version_len] != *version.as_bytes()
        || bytes[199 + version_len..263].iter().any(|b| *b != 0)
        || encoded > ENCODED_BYTES_MAX
        || preview > PREVIEW_BYTES_MAX as u64
        || detail > 4096
        || RESPONSE_HEADER + encoded + preview as usize + detail != bytes.len()
    {
        return Err(failed("Browser DDS response binding/length/version"));
    }
    let encoded_bytes = &bytes[RESPONSE_HEADER..RESPONSE_HEADER + encoded];
    let preview_bytes =
        &bytes[RESPONSE_HEADER + encoded..RESPONSE_HEADER + encoded + preview as usize];
    if bytes[129..161] != *Sha256::digest(encoded_bytes)
        || bytes[161..193] != *Sha256::digest(preview_bytes)
    {
        return Err(failed("Browser DDS result digest mismatch"));
    }
    if bytes[10] != 0 {
        if encoded != 0 || preview != 0 || size != (0, 0) || bytes[193] != 0 || quantized != 0 {
            return Err(failed("Browser DDS refusal carries output"));
        }
        let message = std::str::from_utf8(&bytes[RESPONSE_HEADER..])
            .map_err(|_| failed("Browser DDS refusal UTF-8"))?;
        return Ok(Frame::Refused(Error::new(
            match bytes[10] {
                1 => ErrorCode::CapabilityUnsupported,
                3 => ErrorCode::ResourceLimit,
                4 => ErrorCode::InputSchema,
                5 => ErrorCode::OperationCancelled,
                _ => ErrorCode::CodecFailed,
            },
            message,
        )));
    }
    let mask = bytes[193];
    if (request.source_flags & 4 != 0 && request.bake_intent != 1)
        || detail != 0
        || encoded == 0
        || size != preview_size(request.size)
        || mask > 127
        || (mask & 32 != 0) != request.metadata.omitted()
        || (mask & 3) != (request.source_flags & 3)
        || (mask & 64 != 0) != (request.source_flags & 4 != 0)
        || ((mask & 4) != 0) != (request.settings[0] != 0)
        || (mask & 8 != 0 && (request.settings[4] != 1 || request.settings[3] < 2))
        || quantized as u64 > u64::from(width) * u64::from(height) * 3
        || ((mask & 16) != 0) != (quantized > 0)
        || (request.settings[1] == 0 && quantized != 0)
    {
        return Err(failed("Browser DDS output/statistics profile"));
    }
    let info = inspect_top_mip_checked(encoded_bytes, PIXELS_MAX, &|| Ok(()))?;
    let options = decode_settings(request.settings, request.size)?;
    if (info.image.width, info.image.height) != request.size
        || info.image.encoding != options.encoding
        || info.image.transfer != options.transfer
        || info.image.alpha != options.alpha
        || info.levels.len() != usize::from(request.settings[3])
    {
        return Err(failed("Browser DDS encoded header disagrees with request"));
    }
    Ok(Frame::Success {
        encoded: encoded_bytes,
        preview: preview_bytes,
        preview_size: size,
        losses: LOSSES
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (mask & (1 << i) != 0).then_some(*s))
            .collect(),
        quantized_rgb_samples: quantized,
    })
}
