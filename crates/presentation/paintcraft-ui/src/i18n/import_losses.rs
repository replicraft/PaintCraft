//! Friendly source descriptions shared by image import confirmations.
pub fn import_loss_description(code: &str) -> Option<&'static str> {
    Some(match code {
        "ora-group-structure-not-retained" => {
            "Layer groups, their names, and separate group visibility controls will not be kept. Their current visibility is applied to the layers."
        }
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
        "jpeg-metadata-ignored" => "Original JPEG metadata will not be retained.",
        "jpeg-cmyk-to-rgb" => {
            "Device CMYK/YCCK samples will be converted numerically to RGB without using the source ICC profile. Colors may differ."
        }
        "jpeg-color-model-assumed" => {
            "This JPEG does not declare its CMYK convention. Conventional CMYK will be assumed."
        }
        "png-color-metadata-ignored" => {
            "Other PNG color declarations will not be retained or used for color management."
        }
        "bmp-color-metadata-ignored" => {
            "Other BMP color declarations will not be retained or used for color management."
        }
        "png-metadata-ignored" => "Some PNG metadata will not be retained.",
        "icc-source-retained-unapplied" => {
            "Exact original ICC profile bytes will be kept as source information. Keeping the profile does not change the imported RGB pixels or apply color management."
        }
        "resolution-source-retained-inactive" => {
            "Original image resolution will be kept as source information, not used as active print settings."
        }
        "source-metadata-omitted" => "Original source metadata is omitted from this exported copy.",
        "filters-baked" => crate::filter_bake_export::BAKED_DESCRIPTION,
        "bmp-metadata-ignored" => "Some BMP metadata will not be retained.",
        "resolution-metadata-ignored" => {
            "Image resolution metadata will not be retained; pixel dimensions will stay unchanged."
        }
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
        "jxr-resolution-not-retained" => "The JPEG XR picture's print resolution will not be kept.",
        "icns-metadata-ignored" => {
            "ICNS container metadata will not be kept in the imported picture."
        }
        "icon-density-not-retained-in-flat-document" => {
            "The icon's logical size and Retina density will not be kept in the imported picture."
        }
        "msp-metadata-ignored" => "MSP device settings and header metadata will not be retained.",
        "tiff-other-pages-not-imported" => "Only the selected TIFF page will be imported.",
        "dcx-other-pages-not-imported" => "Only the selected DCX page will be imported.",
        "dds-other-mips-not-imported" => {
            "Only the full-size DDS image will be imported. Its smaller mip levels will stay in the source file."
        }
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
        "psd-metadata-ignored" => {
            "Photoshop metadata and layer protection settings will not be retained in the converted copy."
        }
        "psd-empty-layer-extent-normalized" => {
            "Empty Photoshop layer extents will become at least one transparent pixel in the converted copy."
        }
        "svg-rasterized" => "SVG vectors and text will become pixels in a single picture layer.",
        "svg-font-substituted" => {
            "Unavailable SVG font families will be replaced with bundled fonts."
        }
        "heif-color-reconstruction" | "avif-color-reconstruction" => {
            "Convert YCbCr to 8-bit sRGB with rounding and clipping; 4:2:0 uses bilinear chroma interpolation."
        }
        "heif-item-names-not-retained" => {
            "HEIF image item names are not retained in the new document."
        }
        "avif-item-names-not-retained" => {
            "AVIF image item names are not retained in the new document."
        }
        "hdr-precision-reduced-to-8" => {
            "Floating-point samples will be reduced to 8-bit precision."
        }
        "hdr-tone-mapped" => "Reinhard tone mapping will be applied to each color channel.",
        "hdr-highlights-clipped" => "Highlights outside the display range will be clipped.",
        "hdr-negative-values-clipped" => "Negative color values will be clipped to black.",
        "hdr-source-space-assigned" => {
            "The chosen source color space overrides or supplies the image interpretation."
        }
        "hdr-alpha-convention-assigned" => {
            "The chosen alpha convention overrides the source transparency interpretation."
        }
        "hdr-alpha-flattened" => {
            "Transparency will be flattened onto the chosen color in linear light."
        }
        "hdr-alpha-quantized" => "Transparency will be rounded to 8-bit precision.",
        "hdr-color-space-converted" => "Source colors will be converted to display sRGB.",
        "hdr-metadata-not-retained" => {
            "Original header metadata will not be retained in the new picture."
        }
        "hdr-pixel-aspect-normalized" => {
            "The source pixel aspect ratio will be replaced with square pixels."
        }
        "exr-origin-normalized" => "The source image origin will be moved to the canvas origin.",
        "hdr-rgbe-lower-edge-representative" => {
            "RGBE samples use the lower edge of each stored value interval, not the Radiance midpoint."
        }
        "hdr-linear-alpha-compositing-changed" => {
            "Retained partial transparency will blend in encoded sRGB in this editor, which differs from linear-light source compositing."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn extended_conversion_limitations_are_translated_in_every_language() {
        for code in [
            "jpeg-metadata-ignored",
            "jpeg-cmyk-to-rgb",
            "jpeg-color-model-assumed",
            "png-color-metadata-ignored",
            "bmp-color-metadata-ignored",
            "png-metadata-ignored",
            "icc-source-retained-unapplied",
            "resolution-source-retained-inactive",
            "source-metadata-omitted",
            "filters-baked",
            "bmp-metadata-ignored",
            "resolution-metadata-ignored",
            "dds-other-mips-not-imported",
            "ora-group-structure-not-retained",
            "hdr-precision-reduced-to-8",
            "hdr-tone-mapped",
            "hdr-highlights-clipped",
            "hdr-negative-values-clipped",
            "hdr-source-space-assigned",
            "hdr-alpha-convention-assigned",
            "hdr-alpha-flattened",
            "hdr-alpha-quantized",
            "hdr-color-space-converted",
            "hdr-metadata-not-retained",
            "hdr-pixel-aspect-normalized",
            "exr-origin-normalized",
            "hdr-rgbe-lower-edge-representative",
            "hdr-linear-alpha-compositing-changed",
            "pnm-metadata-ignored",
            "icon-palette-expanded",
            "icon-other-entries-not-imported",
            "cursor-hotspot-not-retained-in-flat-document",
            "pcx-metadata-ignored",
            "msp-metadata-ignored",
            "icns-metadata-ignored",
            "jxr-resolution-not-retained",
            "heif-color-reconstruction",
            "heif-item-names-not-retained",
            "avif-item-names-not-retained",
            "avif-color-reconstruction",
            "icon-density-not-retained-in-flat-document",
            "dcx-other-pages-not-imported",
            "tiff-other-pages-not-imported",
            "pdn-metadata-ignored",
            "ora-opacity-quantized",
            "ora-resolution-ignored",
            "psd-metadata-ignored",
            "psd-empty-layer-extent-normalized",
            "svg-rasterized",
            "svg-font-substituted",
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
