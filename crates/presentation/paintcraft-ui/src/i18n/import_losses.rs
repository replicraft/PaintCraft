//! Friendly source descriptions shared by image import confirmations.
pub fn import_loss_description(code: &str) -> Option<&'static str> {
    Some(match code {
        "dib-alpha-unspecified" => {
            "The DIB has no declared alpha channel; its pixels will be imported as opaque."
        }
        "dib-alpha-mode-unspecified" => {
            "The DIB alpha mode is unspecified. Colors will be interpreted as straight alpha."
        }
        "dib-metadata-ignored" => {
            "Color space and resolution metadata in the DIB will not be retained."
        }
        "depth-16-reduced-to-8" => "High-depth samples will be reduced from 16-bit to 8-bit.",
        "icc-profile-ignored" => "ICC color profiles will not be retained.",
        "exif-metadata-ignored" => "EXIF metadata will not be retained.",
        "animation-first-frame-only" => "Only the first animation frame will be imported.",
        "palette-indices-expanded" => "Palette entries will be expanded to RGBA pixels.",
        "icon-palette-expanded" => "Palette entries will be expanded to RGBA pixels.",
        "icon-other-entries-not-imported" => {
            "Only the selected icon or cursor entry will be imported."
        }
        "cursor-hotspot-not-retained-in-flat-document" => {
            "The cursor hotspot will not be retained in the picture."
        }
        "gif-metadata-ignored" => "GIF metadata will not be retained.",
        "tiff-metadata-ignored" => "TIFF metadata will not be retained.",
        "tga-metadata-ignored" => "TGA metadata will not be retained.",
        "webp-metadata-ignored" => "WebP metadata will not be retained.",
        "pnm-metadata-ignored" => "PNM comments will not be retained.",
        "pcx-metadata-ignored" => {
            "PCX resolution, origin and header metadata will not be retained."
        }
        "dcx-other-pages-not-imported" => "Only the selected DCX page will be imported.",
        "paint-layer-properties-unverified" => {
            "PAINT layer names, visibility and opacity could not be verified. The converted copy uses numbered, visible layers at full opacity."
        }
        "paint-canvas-fill-converted-to-layer" => {
            "The PAINT canvas fill will become a separate background layer in the converted copy."
        }
        "paint-background-depth-reduced-to-8" => {
            "The PAINT background color will be reduced from 16-bit to 8-bit."
        }
        "pdn-metadata-ignored" => {
            "PDN metadata and background-layer lock properties will not be retained in the converted copy."
        }
        "ora-opacity-quantized" => "OpenRaster layer opacity will be rounded to 8-bit precision.",
        "ora-resolution-ignored" => {
            "OpenRaster print resolution will not be retained in the converted copy."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn extended_conversion_limitations_are_translated_in_every_language() {
        for code in [
            "pnm-metadata-ignored",
            "icon-palette-expanded",
            "icon-other-entries-not-imported",
            "cursor-hotspot-not-retained-in-flat-document",
            "pcx-metadata-ignored",
            "dcx-other-pages-not-imported",
            "pdn-metadata-ignored",
            "ora-opacity-quantized",
            "ora-resolution-ignored",
            "paint-layer-properties-unverified",
            "paint-canvas-fill-converted-to-layer",
            "paint-background-depth-reduced-to-8",
        ] {
            let source = super::import_loss_description(code).unwrap();
            for lang in crate::i18n::Language::ALL {
                if lang != crate::i18n::Language::English {
                    assert_ne!(
                        crate::i18n::translate(lang, source),
                        source,
                        "{} {code}",
                        lang.id()
                    );
                }
            }
        }
    }
}
