use super::*;
use ::dds::{Encoder, ImageView, Size};
use std::io::Write;

struct Sink<'a> {
    bytes: Vec<u8>,
    control: &'a Control<'a>,
}
impl Write for Sink<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        (self.control.check)().map_err(|e| self.control.io_failure(e))?;
        if self.bytes.len().saturating_add(bytes.len()) > BYTES_MAX {
            return Err(self
                .control
                .io_failure(resource("DDS exceeds the encoded byte limit")));
        }
        self.bytes.try_reserve_exact(bytes.len()).map_err(|_| {
            self.control
                .io_failure(resource("DDS output allocation failed"))
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        (self.control.check)().map_err(|e| self.control.io_failure(e))
    }
}
pub fn encode_rgba8(width: u32, height: u32, rgba: &[u8], transfer: Transfer) -> Result<Vec<u8>> {
    encode_rgba8_checked(width, height, rgba, transfer, &|| Ok(()))
}
/// Exact packed RGBA8 writer, no mipmaps or BC compression. Source bytes are
/// immutable and no transfer conversion is applied; the caller declares the
/// actual transfer. Unknown transfer must be resolved before choosing a writer.
pub fn encode_rgba8_checked(
    width: u32,
    height: u32,
    rgba: &[u8],
    transfer: Transfer,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let dxgi = match transfer {
        Transfer::Linear => DxgiFormat::R8G8B8A8_UNORM,
        Transfer::Srgb => DxgiFormat::R8G8B8A8_UNORM_SRGB,
        Transfer::Unspecified => {
            return Err(unsupported(
                "DDS export requires an explicit source transfer",
            ));
        }
    };
    let header = Header::new_image(width, height, Format::R8G8B8A8_UNORM)
        .to_dx10()
        .ok_or_else(|| codec("DDS RGBA8 header could not be represented"))?
        .with_dxgi_format(dxgi)
        .with_alpha_mode(AlphaMode::Straight);
    let header = Header::Dx10(header);
    admit(&header, PIXELS_MAX)?;
    let length = usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| resource("DDS output size exceeds target address space"))?;
    if length.checked_add(148).is_none_or(|n| n > BYTES_MAX) {
        return Err(resource("DDS exceeds the output byte limit"));
    }
    if rgba.len() != length {
        return Err(codec("DDS source length does not match dimensions"));
    }
    let control = Control::new(check);
    let mut sink = Sink {
        bytes: Vec::new(),
        control: &control,
    };
    let mut encoder = Encoder::new(&mut sink, Format::R8G8B8A8_UNORM, &header)
        .map_err(|e| control.error(codec(format!("DDS encode header failed: {e}"))))?;
    let view = ImageView::new(rgba, Size::new(width, height), ColorFormat::RGBA_U8)
        .ok_or_else(|| codec("DDS source geometry is invalid"))?;
    encoder
        .write_surface(view)
        .map_err(|e| control.error(codec(format!("DDS encode failed: {e}"))))?;
    encoder
        .finish()
        .map_err(|e| control.error(codec(format!("DDS encode did not finish: {e}"))))?;
    check()?;
    if sink.bytes.len() != length + 148 {
        return Err(codec(
            "DDS encoded length does not match the qualified profile",
        ));
    }
    Ok(sink.bytes)
}
