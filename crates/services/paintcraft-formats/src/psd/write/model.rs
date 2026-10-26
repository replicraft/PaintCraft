use super::*;
use ag_psd::psd::{
    BlendMode, ChannelId, ColorMode, Compression, Layer, LayerRawData, LayerRawDataChannel,
    PixelData, Psd,
};
fn zeroed(count: usize) -> Result<Vec<u8>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| resource("PSD model pixel allocation"))?;
    values.resize(count, 0);
    Ok(values)
}
pub(super) fn build(
    doc: &Document,
    version: Version,
    check: &dyn Fn() -> Result<()>,
) -> Result<Psd> {
    let mut layers = Vec::new();
    layers
        .try_reserve_exact(doc.roots.len())
        .map_err(|_| resource("PSD model layer allocation"))?;
    for id in &doc.roots {
        check()?;
        let layer = &doc.layers[id];
        let width = layer.raster.width();
        let height = layer.raster.height();
        let pixels = width as usize * height as usize;
        let mut planes = [
            zeroed(pixels)?,
            zeroed(pixels)?,
            zeroed(pixels)?,
            zeroed(pixels)?,
        ];
        let mut row = zeroed(width as usize * 4)?;
        for y in 0..height as usize {
            check()?;
            layer.raster.read_span(y as i64, 0, &mut row);
            for (x, pixel) in row.as_chunks::<4>().0.iter().enumerate() {
                for c in 0..4 {
                    planes[c][y * width as usize + x] = pixel[c];
                }
            }
        }
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(4)
            .map_err(|_| resource("PSD channel model allocation"))?;
        for (id, data) in [
            ChannelId::Color0,
            ChannelId::Color1,
            ChannelId::Color2,
            ChannelId::Transparency,
        ]
        .into_iter()
        .zip(planes)
        {
            channels.push(LayerRawDataChannel {
                id,
                compression: Compression::RawData,
                data: Some(data),
            });
        }
        let mut name = String::new();
        name.try_reserve_exact(layer.name.len())
            .map_err(|_| resource("PSD name allocation"))?;
        name.push_str(&layer.name);
        let additional_info = ag_psd::psd::LayerAdditionalInfo {
            name: Some(name),
            ..Default::default()
        };
        layers.push(Layer {
            additional_info,
            top: Some(layer.origin.1 as f64),
            left: Some(layer.origin.0 as f64),
            bottom: Some((layer.origin.1 + i64::from(height)) as f64),
            right: Some((layer.origin.0 + i64::from(width)) as f64),
            blend_mode: Some(BlendMode::Normal),
            opacity: Some(f64::from(layer.opacity) / 255.0),
            hidden: Some(!layer.visible),
            clipping: Some(false),
            transparency_protected: Some(false),
            raw_data: Some(LayerRawData {
                color_mode: ColorMode::Rgb,
                bits_per_channel: 8.0,
                channels,
                large: version == Version::Psb,
            }),
            ..Default::default()
        });
    }
    check()?;
    let merged = paintcraft_render::render_cancellable(doc, check)?;
    check()?;
    Ok(Psd {
        width: f64::from(doc.canvas.0),
        height: f64::from(doc.canvas.1),
        channels: Some(4.0),
        bits_per_channel: Some(8.0),
        color_mode: Some(ColorMode::Rgb),
        children: Some(layers),
        image_data: Some(PixelData {
            width: doc.canvas.0,
            height: doc.canvas.1,
            data: merged,
        }),
        ..Default::default()
    })
}
