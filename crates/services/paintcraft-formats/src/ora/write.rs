//! Fresh raster interchange copies. Callers stage the sink privately and publish
//! only on success. This API does not confer foreign preserving-save authority.
use super::*;
use paintcraft_document::Document;
use std::{collections::HashSet, fmt::Write as _, io::Write};

pub const TITLE_LOSS: &str = "ora-document-title-not-exported";

/// Validate every representable property before writing even the ZIP header.
pub fn preflight(doc: &Document, limits: &Limits) -> Result<Vec<String>> {
    dimensions(doc.canvas.0, doc.canvas.1, limits)?;
    if doc.roots.is_empty() || doc.roots.len() != doc.layers.len() {
        return Err(malformed("ORA requires a nonempty valid flat layer stack"));
    }
    if doc.roots.len() > limits.layers
        || doc.roots.len().saturating_add(4) > limits.entries.min(65534)
    {
        return Err(resource("ORA layer/entry count exceeds writer limits"));
    }
    let mut ids = HashSet::new();
    ids.try_reserve(doc.roots.len())
        .map_err(|_| resource("Cannot allocate ORA identity checks"))?;
    let mut pixels = 0u64;
    for id in &doc.roots {
        let layer = doc
            .layers
            .get(id)
            .filter(|layer| layer.id == *id && ids.insert(*id))
            .ok_or_else(|| malformed("ORA document layer roots are inconsistent"))?;
        if layer.name.chars().count() > 1024 || !layer.name.chars().all(stack::xml_char) {
            return Err(unsupported("ORA layer name cannot be represented as XML"));
        }
        if i32::try_from(layer.origin.0).is_err() || i32::try_from(layer.origin.1).is_err() {
            return Err(unsupported(
                "ORA layer offsets must fit signed 32-bit coordinates",
            ));
        }
        let count = dimensions(layer.raster.width(), layer.raster.height(), limits)?;
        pixels = pixels
            .checked_add(count)
            .ok_or_else(|| resource("ORA layer pixel count overflow"))?;
        if pixels > limits.pixels
            || pixels
                .checked_mul(4)
                .is_none_or(|n| n > limits.decoded_bytes as u64)
        {
            return Err(resource("ORA aggregate layer pixels exceed writer limits"));
        }
    }
    Ok(if doc.title.is_some() {
        vec![TITLE_LOSS.into()]
    } else {
        Vec::new()
    })
}

fn attribute(text: &str, xml: &mut String) {
    for c in text.chars() {
        match c {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '"' => xml.push_str("&quot;"),
            '\t' => xml.push_str("&#9;"),
            '\n' => xml.push_str("&#10;"),
            '\r' => xml.push_str("&#13;"),
            _ => xml.push(c),
        }
    }
}

fn stack_xml(doc: &Document, limits: &Limits, check: &dyn Fn() -> Result<()>) -> Result<String> {
    // Reserve a checked upper bound, including six-byte escapes per input byte.
    let capacity = doc
        .roots
        .iter()
        .try_fold(256usize, |total, id| {
            let layer = &doc.layers[id];
            total.checked_add(layer.name.len().checked_mul(6)?.checked_add(256)?)
        })
        .ok_or_else(|| resource("ORA XML capacity overflow"))?;
    if capacity > limits.xml_bytes || capacity > limits.metadata_bytes {
        return Err(resource("ORA stack XML exceeds writer metadata allowance"));
    }
    let mut xml = String::new();
    xml.try_reserve_exact(capacity)
        .map_err(|_| resource("Cannot allocate ORA XML"))?;
    write!(xml,"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<image version=\"0.0.6\" w=\"{}\" h=\"{}\"><stack>\n",doc.canvas.0,doc.canvas.1).unwrap();
    for (index, id) in doc.roots.iter().enumerate().rev() {
        check()?;
        let layer = &doc.layers[id];
        write!(xml, "<layer src=\"data/layer{index}.png\" name=\"").unwrap();
        attribute(&layer.name, &mut xml);
        writeln!(
            xml,
            "\" x=\"{}\" y=\"{}\" opacity=\"{}\" visibility=\"{}\" composite-op=\"svg:src-over\"/>",
            layer.origin.0,
            layer.origin.1,
            f64::from(layer.opacity) / 255.0,
            if layer.visible { "visible" } else { "hidden" }
        )
        .unwrap();
    }
    xml.push_str("</stack></image>\n");
    // The generated stack must fit the same attribute and XML profile as input.
    stack::parse(
        xml.as_bytes(),
        limits,
        &Budget::new(limits.metadata_bytes),
        check,
    )?;
    Ok(xml)
}

struct Record {
    name: String,
    size: u32,
    crc: u32,
    offset: u32,
}
struct Output<'a, W> {
    sink: W,
    position: usize,
    inflated: usize,
    records: Vec<Record>,
    limits: &'a Limits,
    check: &'a dyn Fn() -> Result<()>,
}
impl<W: Write> Output<'_, W> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self
            .position
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limits.archive_bytes && *n < u32::MAX as usize)
            .ok_or_else(|| resource("ORA encoded archive exceeds byte limit"))?;
        for chunk in bytes.chunks(65536) {
            (self.check)()?;
            self.sink.write_all(chunk).map_err(|_| {
                Error::new(ErrorCode::StorageFailed, "Cannot write staged ORA archive")
            })?;
        }
        self.position = end;
        Ok(())
    }
    fn number16(&mut self, value: u16) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn number32(&mut self, value: u32) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn member(&mut self, name: String, data: &[u8]) -> Result<()> {
        archive::safe_path(&name, self.limits, false)?;
        if name.len() > u16::MAX as usize
            || data.len() > self.limits.entry_bytes
            || data.len() >= u32::MAX as usize
        {
            return Err(resource("ORA member exceeds filename/entry limit"));
        }
        self.inflated = self
            .inflated
            .checked_add(data.len())
            .filter(|n| *n <= self.limits.inflated_bytes)
            .ok_or_else(|| resource("ORA aggregate member bytes exceed allowance"))?;
        let mut crc = crc32fast::Hasher::new();
        for chunk in data.chunks(65536) {
            (self.check)()?;
            crc.update(chunk);
        }
        let record = Record {
            name,
            size: data.len() as u32,
            crc: crc.finalize(),
            offset: self.position as u32,
        };
        self.bytes(b"PK\x03\x04")?;
        for value in [20, 0x800, 0, 0, 33] {
            self.number16(value)?;
        }
        self.number32(record.crc)?;
        self.number32(record.size)?;
        self.number32(record.size)?;
        self.number16(record.name.len() as u16)?;
        self.number16(0)?;
        self.bytes(record.name.as_bytes())?;
        self.bytes(data)?;
        self.records.push(record);
        Ok(())
    }
    fn finish(mut self) -> Result<()> {
        let start = self.position;
        let records = std::mem::take(&mut self.records);
        let directory = records
            .iter()
            .try_fold(0usize, |sum, r| sum.checked_add(46 + r.name.len()))
            .filter(|n| *n <= self.limits.metadata_bytes)
            .ok_or_else(|| resource("ORA directory exceeds metadata allowance"))?;
        if directory
            > self
                .limits
                .metadata_bytes
                .saturating_sub(records.len() * std::mem::size_of::<Record>())
        {
            return Err(resource(
                "ORA directory ownership exceeds metadata allowance",
            ));
        }
        for record in &records {
            self.bytes(b"PK\x01\x02")?;
            for value in [0x314, 20, 0x800, 0, 0, 33] {
                self.number16(value)?;
            }
            self.number32(record.crc)?;
            self.number32(record.size)?;
            self.number32(record.size)?;
            self.number16(record.name.len() as u16)?;
            for _ in 0..4 {
                self.number16(0)?;
            }
            self.number32(0o100644u32 << 16)?;
            self.number32(record.offset)?;
            self.bytes(record.name.as_bytes())?;
        }
        self.bytes(b"PK\x05\x06")?;
        self.number16(0)?;
        self.number16(0)?;
        self.number16(records.len() as u16)?;
        self.number16(records.len() as u16)?;
        self.number32(directory as u32)?;
        self.number32(start as u32)?;
        self.number16(0)?;
        (self.check)()
    }
}

/// Stream a fresh copy into a private sink. A cancelled/failed sink can contain a
/// prefix; the caller must discard it and must never replace the source with it.
pub fn write_document_to(
    doc: &Document,
    sink: impl Write,
    limits: &Limits,
    allowed_losses: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if allowed_losses.iter().any(|loss| loss != TITLE_LOSS) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Unknown OpenRaster export loss acknowledgement",
        ));
    }
    let losses = preflight(doc, limits)?;
    if losses.iter().any(|loss| !allowed_losses.contains(loss)) {
        return Err(unsupported(
            "OpenRaster export would omit the document title; explicit acknowledgement is required",
        ));
    }
    let xml = stack_xml(doc, limits, check)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(doc.roots.len() + 4)
        .map_err(|_| resource("Cannot allocate ORA directory"))?;
    let mut output = Output {
        sink,
        position: 0,
        inflated: 0,
        records,
        limits,
        check,
    };
    output.member("mimetype".into(), b"image/openraster")?;
    output.member("stack.xml".into(), xml.as_bytes())?;
    drop(xml);
    for (index, id) in doc.roots.iter().enumerate() {
        check()?;
        let layer = &doc.layers[id];
        let width = layer.raster.width();
        let height = layer.raster.height();
        let count = usize::try_from(u64::from(width) * u64::from(height) * 4)
            .map_err(|_| resource("ORA pixel bytes exceed address space"))?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(count)
            .map_err(|_| resource("Cannot allocate ORA layer pixels"))?;
        rgba.resize(count, 0);
        for (y, row) in rgba.chunks_exact_mut(width as usize * 4).enumerate() {
            check()?;
            layer.raster.read_span(y as i64, 0, row);
        }
        let png = crate::image::encode_checked(
            crate::image::ImageFormat::Png,
            width,
            height,
            &rgba,
            90,
            check,
        )?;
        drop(rgba);
        output.member(format!("data/layer{index}.png"), &png)?;
    }
    let rgba = paintcraft_render::render_cancellable(doc, check)?;
    let merged = crate::image::encode_checked(
        crate::image::ImageFormat::Png,
        doc.canvas.0,
        doc.canvas.1,
        &rgba,
        90,
        check,
    )?;
    output.member("mergedimage.png".into(), &merged)?;
    // Fit the longest side exactly, preserving aspect ratio without upscaling.
    let max = doc.canvas.0.max(doc.canvas.1);
    let side = max.min(256);
    let width = ((u64::from(doc.canvas.0) * u64::from(side) + u64::from(max) / 2) / u64::from(max))
        .max(1) as u32;
    let height = ((u64::from(doc.canvas.1) * u64::from(side) + u64::from(max) / 2) / u64::from(max))
        .max(1) as u32;
    let image = ::image::RgbaImage::from_raw(doc.canvas.0, doc.canvas.1, rgba)
        .ok_or_else(|| malformed("ORA merged pixel length mismatch"))?;
    check()?;
    let thumbnail = ::image::imageops::resize(
        &image,
        width,
        height,
        ::image::imageops::FilterType::Triangle,
    );
    check()?;
    let png = crate::image::encode_checked(
        crate::image::ImageFormat::Png,
        width,
        height,
        thumbnail.as_raw(),
        90,
        check,
    )?;
    output.member("Thumbnails/thumbnail.png".into(), &png)?;
    output.finish()
}

/// The returned bytes exist only after all writes and cancellation checks succeed.
pub fn encode_document(
    doc: &Document,
    limits: &Limits,
    allowed_losses: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    struct Sink(Vec<u8>);
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .try_reserve(bytes.len())
                .map_err(|_| std::io::Error::other("ORA allocation failed"))?;
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink(Vec::new());
    write_document_to(doc, &mut sink, limits, allowed_losses, check)?;
    Ok(sink.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_is_full_canvas_and_thumbnail_fits_without_upscaling() {
        for (width, height, expected) in [
            (1000, 486, (256, 124)),
            (486, 1000, (124, 256)),
            (17, 9, (17, 9)),
            (1000, 1, (256, 1)),
        ] {
            let doc = paintcraft_document::new_document(
                width,
                height,
                Some(paintcraft_core::types::Rgba([17, 51, 93, 255])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap();
            let limits = Limits::default();
            let bytes = encode_document(&doc, &limits, &[], &|| Ok(())).unwrap();
            let archive = archive::Archive::parse(
                &bytes,
                &limits,
                &Budget::new(limits.metadata_bytes),
                &|| Ok(()),
            )
            .unwrap();
            for (name, expected) in [
                ("mergedimage.png", (width, height)),
                ("Thumbnails/thumbnail.png", expected),
            ] {
                let png = archive
                    .read(archive.index(name).unwrap(), &limits, &[], &|| Ok(()))
                    .unwrap();
                let image = crate::image::decode(&png).unwrap();
                assert_eq!((image.width, image.height), expected);
                assert!(image.rgba.chunks_exact(4).all(|px| px == [17, 51, 93, 255]));
            }
        }
    }
}
