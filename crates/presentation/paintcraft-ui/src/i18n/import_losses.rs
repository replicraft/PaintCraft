//! Friendly source descriptions shared by image import confirmations.
pub fn import_loss_description(code: &str) -> Option<&'static str> {
    Some(match code {
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
