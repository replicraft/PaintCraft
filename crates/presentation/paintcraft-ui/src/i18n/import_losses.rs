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
        "gif-metadata-ignored" => "GIF metadata will not be retained.",
        "tiff-metadata-ignored" => "TIFF metadata will not be retained.",
        "tga-metadata-ignored" => "TGA metadata will not be retained.",
        "webp-metadata-ignored" => "WebP metadata will not be retained.",
        _ => return None,
    })
}
