//! AVIF property semantics atop the existing shared bounded association reader.
use super::*;
use crate::item_graph::{Reader, full, invalid, resource, unsupported};
use paintcraft_paint::BoxInfo;
#[derive(Clone, Debug)]
pub(super) enum Property<'a> {
    Spatial {
        width: u32,
        height: u32,
    },
    Pixels(&'a [u8]),
    Av1(Av1Configuration<'a>),
    Nclx {
        primaries: u16,
        transfer: u16,
        matrix: u16,
        full_range: bool,
    },
    Auxiliary {
        kind: &'a str,
        subtype: &'a [u8],
    },
    Unsupported([u8; 4]),
}
pub(super) fn parse<'a>(info: &BoxInfo<'a>, pixels: u64) -> Result<Property<'a>> {
    let data = info.payload;
    let mut reader = Reader::new(data);
    match &info.kind {
        b"ispe" => {
            let (version, flags, body) = full(data)?;
            if version != 0 || flags != 0 {
                return Err(unsupported("AVIF ispe version/flags"));
            }
            reader = Reader::new(body);
            let width = reader.u32()?;
            let height = reader.u32()?;
            reader.end()?;
            if width == 0
                || height == 0
                || width > 65536
                || height > 65536
                || u64::from(width) * u64::from(height) > pixels.min(INSPECT_PIXELS_MAX)
            {
                return Err(resource("AVIF associated dimensions exceed allowance"));
            }
            Ok(Property::Spatial { width, height })
        }
        b"pixi" => {
            let (version, flags, body) = full(data)?;
            if version != 0 || flags != 0 {
                return Err(unsupported("AVIF pixi version/flags"));
            }
            reader = Reader::new(body);
            let count = reader.u8()? as usize;
            if !matches!(count, 1 | 3) {
                return Err(unsupported("AVIF pixi channel count"));
            }
            let bits = reader.take(count)?;
            reader.end()?;
            if bits.iter().any(|bit| !matches!(bit, 8 | 10 | 12)) {
                return Err(unsupported("AVIF pixi depth"));
            }
            Ok(Property::Pixels(bits))
        }
        b"av1C" => {
            let configuration = inspect::av1_configuration(0, data)?;
            if data[3] != 0 || !configuration.config_obus.is_empty() {
                return Err(unsupported("AVIF configuration OBUs/delay are unqualified"));
            }
            Ok(Property::Av1(configuration))
        }
        b"colr" => {
            if reader.take(4)? != b"nclx" {
                return Err(unsupported("AVIF ICC/custom color profile is unqualified"));
            }
            let primaries = reader.u16()?;
            let transfer = reader.u16()?;
            let matrix = reader.u16()?;
            let range = reader.u8()?;
            reader.end()?;
            if range & 0x7f != 0 {
                return Err(invalid("AVIF nclx reserved bits"));
            }
            Ok(Property::Nclx {
                primaries,
                transfer,
                matrix,
                full_range: range & 0x80 != 0,
            })
        }
        b"auxC" => {
            let (version, flags, body) = full(data)?;
            if version != 0 || flags != 0 {
                return Err(unsupported("AVIF auxC version/flags"));
            }
            reader = Reader::new(body);
            let kind = reader.string()?;
            let subtype = reader.take(body.len() - reader.pos)?;
            Ok(Property::Auxiliary { kind, subtype })
        }
        _ => Ok(Property::Unsupported(info.kind)),
    }
}
