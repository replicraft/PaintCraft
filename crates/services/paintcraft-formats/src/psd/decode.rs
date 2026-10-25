use super::*;
use ag_psd::psd::{BlendMode, ColorMode, PixelData, ReadOptions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedLayer {
    pub info: LayerInfo,
    /// Straight RGBA8, with hidden RGB retained.
    pub rgba: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// Background to foreground, matching independent psd-tools record order.
    pub layers: Vec<DecodedLayer>,
    pub losses: Vec<String>,
}

/// Decode the preflighted RGB8 raster subset. Layered sources use actual layer
/// pixels, never the merged preview. Layerless sources get one composite layer.
/// Publication, loss consent and source preservation belong to the importer.
///
/// The backend has no cancellation hook inside a single channel decode; this
/// adapter checks before/after library work and between bounded layer decodes.
pub fn decode(bytes: &[u8], pixel_limit: u64, check: &dyn Fn() -> Result<()>) -> Result<Decoded> {
    let inspection = inspect(bytes, pixel_limit, check)?;
    let layered = !inspection.layers.is_empty();
    // Bound and inspect resources ourselves, report losses, then omit them from
    // the private backend view. Unknown descriptors and thumbnail decoders must
    // never bypass admission merely because they are not pixel layer data.
    let mut input = Vec::new();
    let size = bytes.len() - inspection.resources.len() + 4;
    input
        .try_reserve_exact(size)
        .map_err(|_| resource("cannot allocate private PSD decoder view"))?;
    input.extend_from_slice(&bytes[..inspection.resources.start]);
    input.extend_from_slice(&0u32.to_be_bytes());
    input.extend_from_slice(&bytes[inspection.resources.end..]);
    normalize_composite(bytes, &inspection, layered, &mut input, check)?;
    check()?;
    let options = ReadOptions {
        use_image_data: Some(true),
        use_raw_data: Some(layered),
        skip_composite_image_data: Some(layered),
        skip_layer_image_data: Some(!layered),
        skip_thumbnail: Some(true),
        skip_linked_files_data: Some(true),
        strict: Some(true),
        throw_for_missing_features: Some(true),
        total_memory_limit: Some(PIXELS_MAX as usize * 8),
        ..ReadOptions::default()
    };
    let mut source = ag_psd::read_psd(&input, &options)
        .map_err(|error| malformed(&format!("PSD backend read failed: {error}")))?;
    drop(input);
    check()?;
    if source.width != f64::from(inspection.width)
        || source.height != f64::from(inspection.height)
        || source.color_mode != Some(ColorMode::Rgb)
        || source.bits_per_channel != Some(8.0)
    {
        return Err(malformed(
            "PSD backend header disagrees with admitted structure",
        ));
    }
    let mut layers = Vec::new();
    layers
        .try_reserve_exact(inspection.layers.len().max(1))
        .map_err(|_| resource("cannot allocate decoded PSD layer inventory"))?;
    if layered {
        let children = source.children.take().unwrap_or_default();
        if children.len() != inspection.layers.len() {
            return Err(malformed("PSD backend changed admitted layer count"));
        }
        for (layer, mut info) in children.into_iter().zip(inspection.layers) {
            check()?;
            if layer.children.is_some()
                || layer.blend_mode != Some(BlendMode::Normal)
                || layer.left != Some(f64::from(info.origin.0))
                || layer.top != Some(f64::from(info.origin.1))
                || layer.right != Some(f64::from(info.origin.0) + f64::from(info.width))
                || layer.bottom != Some(f64::from(info.origin.1) + f64::from(info.height))
                || layer.opacity != Some(f64::from(info.opacity) / 255.0)
                || layer.hidden.unwrap_or(false) == info.visible
            {
                return Err(malformed("PSD backend changed admitted layer semantics"));
            }
            let pixels = if info.width == 0 || info.height == 0 {
                info.width = info.width.max(1);
                info.height = info.height.max(1);
                PixelData {
                    width: info.width,
                    height: info.height,
                    data: vec![0; info.width as usize * info.height as usize * 4],
                }
            } else {
                ag_psd::get_layer_image_data(&layer)
                    .map_err(|error| malformed(&format!("PSD layer decode failed: {error}")))?
                    .ok_or_else(|| malformed("PSD backend omitted admitted raster pixels"))?
            };
            check()?;
            validate(&pixels, info.width, info.height)?;
            layers.push(DecodedLayer {
                info,
                rgba: pixels.data,
            });
        }
    } else {
        // Eager decoding retains PSB row-length widths. The backend's lazy
        // composite helper loses this information and must not be used here.
        let pixels = source
            .image_data
            .take()
            .ok_or_else(|| malformed("PSD backend omitted admitted composite pixels"))?;
        validate(&pixels, inspection.width, inspection.height)?;
        let info = LayerInfo {
            name: "Background".into(),
            origin: (0, 0),
            width: inspection.width,
            height: inspection.height,
            opacity: 255,
            visible: true,
            source_id: None,
            channels: Vec::new(),
        };
        layers.push(DecodedLayer {
            info,
            rgba: pixels.data,
        });
    }
    check()?;
    Ok(Decoded {
        width: inspection.width,
        height: inspection.height,
        layers,
        losses: inspection.losses,
    })
}

fn normalize_composite(
    bytes: &[u8],
    inspection: &Inspection,
    layered: bool,
    input: &mut Vec<u8>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let at = inspection.composite;
    let compression = u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap());
    if layered || compression < 2 {
        return Ok(());
    }
    // ag-psd 0.3's whole-file composite reader expects a separate ZIP stream
    // for each channel. Independent psd-tools fixtures use one stream for all
    // planes. Decode that admitted stream with the selected library's channel
    // helper, then feed an equivalent raw composite to its structural reader.
    // This private representation change never rewrites the source file.
    check()?;
    let width = inspection.width as usize;
    let rows = inspection.height as usize * 3;
    let mut planar = ag_psd::reader::DecodeTarget::wide(width, rows, 1);
    ag_psd::reader::read_data_zip(
        &bytes[at + 2..],
        Some(&mut planar),
        width,
        rows,
        8,
        1,
        0,
        compression == 3,
    );
    check()?;
    input.truncate(at - inspection.resources.len() + 4);
    input
        .try_reserve_exact(planar.data.len() + 2)
        .map_err(|_| resource("cannot allocate normalized PSD composite view"))?;
    input.extend_from_slice(&0u16.to_be_bytes());
    input.extend_from_slice(&planar.data);
    Ok(())
}

fn validate(pixels: &PixelData, width: u32, height: u32) -> Result<()> {
    if pixels.width != width
        || pixels.height != height
        || pixels.data.len() as u64 != u64::from(width) * u64::from(height) * 4
    {
        return Err(malformed(
            "PSD decoded pixels contradict admitted dimensions",
        ));
    }
    Ok(())
}
