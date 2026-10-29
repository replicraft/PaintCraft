//! Whole immutable TIFF source, inventory and selected decode stay in the Worker.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::tiff_pages::{self, browser_wire as wire};
pub fn execute(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < wire::REQUEST_HEADER || bytes.len() > wire::REQUEST_HEADER + wire::SOURCE_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "TIFF Worker input allowance",
        ));
    }
    let header = wire::Header::decode(&bytes[..wire::REQUEST_HEADER])?;
    let source = &bytes[wire::REQUEST_HEADER..];
    header.verify_source(source)?;
    match prepare(header, source) {
        Ok(output) => Ok(output),
        Err(error) => wire::refusal(&header, env!("CARGO_PKG_VERSION"), &error),
    }
}
fn prepare(header: wire::Header, source: &[u8]) -> Result<Vec<u8>> {
    let inventory = tiff_pages::inspect(source, &|| Ok(()))?;
    let count = u16::try_from(inventory.pages().len())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "TIFF page count"))?;
    if let Some(selection) = header.selection {
        let page = inventory
            .pages()
            .get(selection.page.index as usize)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ConflictStaleBase,
                    "TIFF selected page disappeared",
                )
            })?;
        if count != selection.page_count
            || inventory.source_sha256() != selection.source_sha256
            || page.index != selection.page.index
            || page.width != selection.page.width
            || page.height != selection.page.height
            || page.descriptor.ifd_offset != selection.page.ifd_offset
            || page.unsupported_reason.is_some()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "TIFF inventory changed before selected decode",
            ));
        }
        let decoded =
            tiff_pages::decode_page(source, page.index, u64::from(header.pixels), &|| Ok(()))?;
        decoded.assessment.validate_result(
            header.source_sha256,
            header.source_bytes,
            page.index,
            decoded.width,
            decoded.height,
            &decoded.rgba,
        )?;
        if decoded.assessment.page_count() != count
            || decoded.assessment.descriptor().ifd_offset != selection.page.ifd_offset
        {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "TIFF selected assessment changed",
            ));
        }
        wire::response(
            &header,
            env!("CARGO_PKG_VERSION"),
            count,
            decoded.width,
            decoded.height,
            &decoded.assessment.encode(),
            &decoded.rgba,
        )
    } else {
        let pages: Vec<_> = inventory
            .pages()
            .iter()
            .map(|page| wire::Page {
                index: page.index,
                width: page.width,
                height: page.height,
                ifd_offset: page.descriptor.ifd_offset,
                supported: page.unsupported_reason.is_none(),
            })
            .collect();
        let payload = wire::encode_inventory(&pages)?;
        wire::response(
            &header,
            env!("CARGO_PKG_VERSION"),
            count,
            0,
            0,
            &[],
            &payload,
        )
    }
}

#[cfg(test)]
mod tests;
