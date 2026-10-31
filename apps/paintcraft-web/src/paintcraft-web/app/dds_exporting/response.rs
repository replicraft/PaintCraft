//! Full Rust validation precedes display/download; JS slices alone are not proof.
use super::*;
#[cfg(test)]
pub(super) fn validate(
    request: &wire::Header,
    echo: &[u8],
    response: Vec<u8>,
    on_validated: impl FnOnce(),
) -> Result<Prepared> {
    validate_request(&Request::V4(request.clone()), echo, response, on_validated)
}
pub(super) fn validate_request(
    request: &Request,
    echo: &[u8],
    response: Vec<u8>,
    on_validated: impl FnOnce(),
) -> Result<Prepared> {
    request.validate_echo(echo)?;
    let parts = match request {
        Request::V4(h) => {
            match wire::decode_response(&response, h, h.owner, env!("CARGO_PKG_VERSION"))? {
                wire::Frame::Success {
                    encoded,
                    preview,
                    preview_size,
                    losses,
                    ..
                } => Ok((encoded, preview, preview_size, losses)),
                wire::Frame::Refused(error) => Err(error),
            }
        }
        Request::V5(h) => {
            match wire::v5::decode_response(&response, h, h.owner, env!("CARGO_PKG_VERSION"))? {
                wire::v5::Frame::Success {
                    encoded,
                    preview,
                    preview_size,
                    losses,
                    ..
                } => Ok((encoded, preview, preview_size, losses)),
                wire::v5::Frame::Refused(error) => Err(error),
            }
        }
        Request::V6(h) => {
            match wire::v6::decode_response(&response, h, h.owner, env!("CARGO_PKG_VERSION"))? {
                wire::v6::Frame::Success {
                    encoded,
                    preview,
                    preview_size,
                    losses,
                    ..
                } => Ok((encoded, preview, preview_size, losses)),
                wire::v6::Frame::Refused(error) => Err(error),
            }
        }
    };
    on_validated();
    let (encoded, preview, preview_size, losses) = parts?;
    let start = request.response_header();
    let end = start + encoded.len();
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(preview.len() / 4)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "DDS preview allocation"))?;
    for pixel in preview.as_chunks::<4>().0 {
        let mut pixel = *pixel;
        if request.settings()[1] == 1 {
            for channel in &mut pixel[..3] {
                *channel = egui::ecolor::gamma_u8_from_linear_f32(f32::from(*channel) / 255.0);
            }
        }
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
