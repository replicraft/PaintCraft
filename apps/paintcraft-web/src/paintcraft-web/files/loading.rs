//! Input admission and private candidates under the browser-owned metadata realm.
use super::*;
use crate::editor::MAX_PIXELS;
use sha2::{Digest, Sha256};

pub(super) fn decode_input_with_metadata(
    bytes: &[u8],
    name: String,
    frontend: Frontend,
    convert: bool,
    metadata: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Loaded> {
    check()?;
    if bytes.len() > FILE_BYTES {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Browser input limit is 64 MiB",
        ));
    }
    let family = if convert {
        paintcraft_formats::registry::admit_import(frontend, &name, bytes, Platform::Web)?
    } else {
        paintcraft_formats::registry::admit_open(frontend, &name, bytes, Platform::Web)?
    };
    if convert && family.is_project() && !paintcraft_formats::project_import::supports(family) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Import image expects a raster picture",
        ));
    }
    if paintcraft_formats::project_import::supports(family) {
        let converted = if family == Family::Ora && convert {
            paintcraft_formats::registry::admit_import_ora(frontend, &name, bytes, Platform::Web)?;
            paintcraft_formats::project_import::decode_ora_with_neutral_groups(
                bytes, MAX_PIXELS, check,
            )?
        } else {
            paintcraft_formats::project_import::decode_layered(family, bytes, MAX_PIXELS, check)?
        };
        return Ok(Loaded {
            raster_preview: false,
            converted_copy: true,
            document: converted.document,
            name,
            losses: converted.losses,
        });
    }
    if family == Family::Pcnd {
        return Ok(Loaded {
            raster_preview: false,
            converted_copy: false,
            document: pcnd::read_with_admission_checked(
                bytes,
                MAX_PIXELS,
                MAX_PIXELS * 4,
                paintcraft_document::LAYER_COUNT_MAX,
                pcnd::ReadAdmission {
                    metadata: pcnd::MetadataAdmission {
                        bytes: paintcraft_document::source_metadata::BYTES_MAX,
                        budget: metadata,
                    },
                    coverage: pcnd::CoverageAdmission {
                        bytes: 256 << 20,
                        budget: &paintcraft_core::types::resources::default_coverage_budget(),
                        limits: paintcraft_raster::coverage_mask::Limits::default(),
                    },
                },
                check,
            )?,
            name,
            losses: Vec::new(),
        });
    }
    if matches!(family, Family::Png | Family::Bmp) {
        let decode = if family == Family::Png {
            image::png_retention::decode
        } else {
            image::bmp_retention::decode
        };
        let prepared = decode(
            bytes,
            MAX_PIXELS,
            pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: metadata,
            },
            check,
        )?;
        let mut admission = paintcraft_raster::Admission::new(MAX_PIXELS * 4);
        let document = prepared.document_with_admission(&mut admission, check)?;
        return Ok(Loaded {
            raster_preview: true,
            converted_copy: convert
                || prepared.attachment.is_some()
                || !prepared.image.losses.is_empty(),
            document,
            name,
            losses: prepared.image.losses,
        });
    }
    let image = image::decode_with_pixel_limit(bytes, MAX_PIXELS)?;
    check()?;
    let key = image::document_conversion_key(bytes, &image, check)?
        .map(|key| key.hex())
        .unwrap_or_else(|| paintcraft_document::hex(&Sha256::digest(bytes)));
    Ok(Loaded {
        raster_preview: true,
        converted_copy: convert || !image.losses.is_empty(),
        document: paintcraft_document::import_image_checked(
            image.width,
            image.height,
            &image.rgba,
            &key,
            check,
        )?,
        name,
        losses: image.losses,
    })
}

#[cfg(test)]
mod mask_tests {
    use super::*;
    #[test]
    fn browser_pcnd4_open_keeps_masks_and_literal_source_in_all_frontends() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/services/paintcraft-formats/tests/pcnd-mask-fixtures/opaque-half.pcnd"
        ));
        let metadata = SharedBudget::new(0);
        for frontend in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
            Frontend::Studio,
        ] {
            let loaded = decode_input_with_metadata(
                bytes,
                "source.pcnd".into(),
                frontend,
                false,
                &metadata,
                &|| Ok(()),
            )
            .unwrap();
            assert!(!loaded.converted_copy);
            assert!(!loaded.raster_preview);
            assert!(loaded.losses.is_empty());
            let doc = &loaded.document;
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(
                layer.filters.as_slice()[0]
                    .mask()
                    .unwrap()
                    .coverage()
                    .coverage(0, 0),
                128
            );
            assert_eq!(
                layer.raster.get(0, 0),
                paintcraft_core::types::Rgba([255, 0, 0, 255])
            );
            assert_eq!(pcnd::write_compact(doc).unwrap(), bytes);
        }
    }
}
