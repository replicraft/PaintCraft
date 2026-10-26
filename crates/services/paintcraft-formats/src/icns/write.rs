use super::*;
use std::io::{self, Write};
#[derive(Clone, Copy, Debug)]
pub struct WriteEntry<'a> {
    pub kind: IconType,
    pub rgba: &'a [u8],
}
struct Output<'a> {
    bytes: Vec<u8>,
    maximum: usize,
    failure: Option<Error>,
    check: &'a dyn Fn() -> Result<()>,
}
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|end| end > self.maximum)
            {
                return Err(limit("ICNS writer exceeded admitted output length"));
            }
            for chunk in bytes.chunks(65536) {
                (self.check)()?;
                self.bytes.extend_from_slice(chunk);
            }
            Ok(bytes.len())
        })();
        result.map_err(|error| {
            self.failure.get_or_insert(error);
            io::Error::other("ICNS output rejected")
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Fresh PNG-only ICNS output, preserving requested logical size/Retina type and
/// exact supplied RGBA (including hidden RGB). No resize/type choice is implicit.
pub fn encode_entries(
    entries: &[WriteEntry<'_>],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    if entries.is_empty() || entries.len() > IconType::WRITABLE.len() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "ICNS output needs 1..8 distinct modern PNG types",
        ));
    }
    let mut pixels = 0u64;
    for (index, entry) in entries.iter().enumerate() {
        if !entry.kind.writable() {
            return Err(unsupported(
                "ICNS PNG output at 16/32/64 logical pixels and 1x lacks exact native readback; select an explicitly qualified type",
            ));
        }
        if entries[..index].iter().any(|old| old.kind == entry.kind) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "duplicate ICNS output type",
            ));
        }
        let side = entry.kind.pixel_size();
        let count = u64::from(side) * u64::from(side);
        pixels = pixels
            .checked_add(count)
            .filter(|count| *count <= PIXELS_MAX)
            .ok_or_else(|| limit("ICNS writer aggregate pixel allowance exceeded"))?;
        if entry.rgba.len() as u64 != count * 4 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "ICNS RGBA length disagrees with selected type",
            ));
        }
    }
    let mut family = ::icns::IconFamily::new();
    let mut total = 8 + 8 + entries.len() * 8;
    let mut toc = allocation(entries.len() * 8)?;
    for entry in entries {
        check()?;
        let side = entry.kind.pixel_size();
        let png = crate::image::encode_checked(
            crate::image::ImageFormat::Png,
            side,
            side,
            entry.rgba,
            90,
            check,
        )?;
        if png.len() > ENTRY_BYTES_MAX {
            return Err(limit("ICNS encoded PNG allowance exceeded"));
        }
        crate::icons::inspect_icns_png(&png, side, side)?;
        total = total
            .checked_add(png.len() + 8)
            .filter(|total| *total <= BYTES_MAX)
            .ok_or_else(|| limit("ICNS output byte allowance exceeded"))?;
        toc.extend(entry.kind.tag());
        toc.extend(((png.len() + 8) as u32).to_be_bytes());
        family.elements.push(::icns::IconElement::new(
            ::icns::OSType(entry.kind.tag()),
            png,
        ));
    }
    family
        .elements
        .insert(0, ::icns::IconElement::new(::icns::OSType(*b"TOC "), toc));
    let mut output = Output {
        bytes: allocation(total)?,
        maximum: total,
        failure: None,
        check,
    };
    let result = family.write(&mut output);
    if let Some(error) = output.failure {
        return Err(error);
    }
    result.map_err(|_| bad("selected ICNS writer failed"))?;
    if output.bytes.len() != total {
        return Err(bad("selected ICNS writer produced unexpected length"));
    }
    inspect(&output.bytes, check)?;
    Ok(output.bytes)
}
