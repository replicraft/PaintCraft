//! Full Rust validation precedes display/download; JS slices alone are not proof.
use super::*;
pub(super) fn validate(
    request: &wire::Header,
    echo: &[u8],
    response: Vec<u8>,
    on_validated: impl FnOnce(),
) -> Result<Prepared> {
    if wire::Header::decode(echo)? != *request {
        return Err(failed(
            "MSP2 request echo differs from captured source/options",
        ));
    }
    let frame =
        wire::decode_response(&response, request, request.owner, env!("CARGO_PKG_VERSION"))?;
    on_validated();
    let wire::Frame::Success {
        encoded,
        preview,
        preview_size,
        losses,
        ..
    } = frame
    else {
        let wire::Frame::Refused(error) = frame else {
            unreachable!()
        };
        return Err(error);
    };
    let start = wire::RESPONSE_HEADER;
    let end = start + encoded.len();
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(preview.len() / 4)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "MSP2 preview allocation"))?;
    for pixel in preview.as_chunks::<4>().0 {
        let pixel = *pixel;
        pixels.push(egui::Color32::from_rgba_unmultiplied(
            pixel[0], pixel[1], pixel[2], pixel[3],
        ));
    }
    let losses = losses.into_iter().map(str::to_owned).collect();
    Ok(Prepared {
        header: request.clone(),
        response,
        encoded: start..end,
        preview: Some(egui::ColorImage::new(
            [preview_size.0 as usize, preview_size.1 as usize],
            pixels,
        )),
        losses,
        #[cfg(target_arch = "wasm32")]
        lease: None,
    })
}
