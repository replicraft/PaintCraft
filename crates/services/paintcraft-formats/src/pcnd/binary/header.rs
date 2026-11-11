use super::*;
pub(super) fn header(version: u8) -> Header {
    let known = matches!(version, 1..=9);
    Header {
        container_version: i64::from(version),
        document_schema: if known {
            if version == 1 {
                "paintcraft.document/v1"
            } else if version == 2 {
                "paintcraft.document/v2"
            } else if version == 3 {
                "paintcraft.document/v3"
            } else if version == 4 {
                "paintcraft.document/v4"
            } else if version == 5 {
                "paintcraft.document/v5"
            } else if version == 6 {
                "paintcraft.document/v6"
            } else if version == 7 {
                "paintcraft.document/v7"
            } else if version == 8 {
                "paintcraft.document/v8"
            } else {
                "paintcraft.document/v9"
            }
        } else {
            "unknown"
        }
        .into(),
        required_features: if known {
            SUPPORTED_FEATURES
                .iter()
                .map(|feature| (*feature).into())
                .chain((version == 2).then_some(SOURCE_METADATA_FEATURE.to_owned()))
                .chain((version >= 3).then_some(LAYER_FILTERS_FEATURE.to_owned()))
                .chain((version >= 4).then_some(LAYER_MASKS_FEATURE.to_owned()))
                .chain((version == 5).then_some(LAYER_SPATIAL_FILTERS_FEATURE.to_owned()))
                .chain((version >= 6).then_some(LAYER_CATALOG_FEATURE.to_owned()))
                .chain((version >= 7).then_some(LAYER_RASTER_MASKS_FEATURE.to_owned()))
                .chain((version >= 8).then_some(LAYER_BLEND_MODES_FEATURE.to_owned()))
                .chain((version == 9).then_some(LAYER_STYLES_FEATURE.to_owned()))
                .collect()
        } else {
            Vec::new()
        },
    }
}
