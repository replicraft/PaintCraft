//! Serialization and published schema share private typed report definitions.
use super::*;
use paintcraft_formats::{
    image::{bmp_metadata, jpeg, png_metadata, raster_source_metadata as facts},
    source_inspection::Source,
};
#[path = "json/types.rs"]
mod types;
use types as dto;
fn digest(value: &facts::Digest) -> dto::Digest {
    dto::Digest {
        bytes: value.bytes,
        sha256: hex(&value.sha256),
    }
}
fn icc(value: &Option<facts::Icc>) -> Option<dto::Icc> {
    value.as_ref().map(|value| dto::Icc {
        payload: digest(&value.payload),
        color_space_signature: value.color_space,
        connection_space_signature: value.connection_space,
        tag_count: value.tag_count,
        interpretation: "container-header-only; no profile assignment or color conversion",
    })
}
fn resolution(value: &Option<facts::Resolution>) -> Option<dto::Resolution> {
    value.map(|value| dto::Resolution {
        x: value.x,
        y: value.y,
        unit: match value.unit {
            facts::ResolutionUnit::Unknown => "unknown",
            facts::ResolutionUnit::Meter => "meter",
        },
    })
}
fn palette(value: &facts::Palette) -> dto::Palette {
    dto::Palette {
        rgb: value.rgb.clone(),
        alpha: value.alpha.clone(),
        indices_retained: false,
    }
}
fn png(s: &png_metadata::Descriptor) -> dto::Png {
    dto::Png {
        width: s.width,
        height: s.height,
        component_depth: s.bit_depth,
        components: match s.color_type {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => 0,
        },
        color_type: s.color_type,
        sample_type: if s.color_type == 3 {
            "palette-index"
        } else {
            "unsigned-integer"
        },
        sample_byte_order: "big-endian/packed",
        interlaced: s.interlaced,
        packed_row_bytes: s.packed_row_bytes(),
        palette: palette(&s.palette),
        transparency_samples: s.transparency.clone(),
        icc: icc(&s.icc),
        resolution: resolution(&s.resolution),
        gamma_scaled: s.gamma_scaled,
        chromaticities_scaled: s.chromaticities_scaled,
        srgb_intent: s.srgb_intent,
        cicp: s.cicp,
        mastering_display: s.mastering_display,
        content_light: s.content_light,
        color_declaration_precedence: match s.color_declaration() {
            png_metadata::ColorDeclaration::Unspecified => "unspecified",
            png_metadata::ColorDeclaration::GammaChromaticities => "gamma-chromaticities",
            png_metadata::ColorDeclaration::Srgb => "srgb",
            png_metadata::ColorDeclaration::Icc => "icc",
            png_metadata::ColorDeclaration::Cicp => "cicp",
        },
        exif_historical_payload: s.exif.as_ref().map(digest),
        exif_orientation: None,
        significant_bits: s.significant_bits.clone(),
        omitted_ancillary_count: s.omitted_ancillary_count,
        omitted_ancillary_bytes: s.omitted_ancillary_bytes,
        omitted_ancillary_sha256: hex(&s.omitted_ancillary_sha256),
        other_metadata: s.other_metadata,
    }
}
fn bmp_depths(s: &bmp_metadata::Descriptor) -> Vec<u8> {
    if s.bits_per_pixel <= 8 {
        vec![s.bits_per_pixel as u8]
    } else if s.bits_per_pixel == 16 {
        if s.compression == 0 {
            vec![5, 5, 5]
        } else {
            s.masks[..3]
                .iter()
                .map(|mask| mask.count_ones() as u8)
                .collect()
        }
    } else {
        vec![8; if s.masks[3] == 0 { 3 } else { 4 }]
    }
}
fn bmp(s: &bmp_metadata::Descriptor) -> dto::Bmp {
    dto::Bmp {
        width: s.width,
        height: s.height,
        top_down: s.top_down,
        bits_per_pixel: s.bits_per_pixel,
        component_depth: if s.bits_per_pixel == 16 {
            let d = bmp_depths(s);
            (d[0] == d[1] && d[1] == d[2]).then_some(u16::from(d[0]))
        } else {
            Some(if s.bits_per_pixel <= 8 {
                s.bits_per_pixel
            } else {
                8
            })
        },
        component_depths: bmp_depths(s),
        components: if s.bits_per_pixel <= 8 {
            1
        } else if s.masks[3] != 0 {
            4
        } else {
            3
        },
        alpha_declaration: if s.masks[3] != 0 {
            "explicit-mask"
        } else {
            "not-declared"
        },
        sample_type: if s.bits_per_pixel <= 8 {
            "palette-index"
        } else {
            "packed-unsigned-integer"
        },
        sample_byte_order: "little-endian/packed",
        dib_bytes: s.dib_bytes,
        compression: s.compression,
        row_stride: s.row_stride,
        declared_image_bytes: s.declared_image_bytes,
        pixel_data_bytes: s.pixel_data_bytes,
        rle_stream_bytes: s.rle_stream_bytes,
        pixel_offset: s.pixel_offset,
        masks: s.masks,
        palette: palette(&s.palette),
        palette_reserved: s.palette_reserved.clone(),
        palette_reserved_is_alpha: false,
        declared_colors_used: s.declared_colors_used,
        important_colors: s.important_colors,
        file_reserved: s.file_reserved,
        resolution_raw: s.resolution_raw,
        resolution: resolution(&s.resolution),
        color_space: s.color_space,
        endpoints_raw: s.endpoints_raw,
        gamma_scaled: s.gamma_scaled,
        rendering_intent: s.rendering_intent,
        icc: icc(&s.icc),
        undeclared_bytes: s.undeclared_bytes,
    }
}
fn jpeg(s: &jpeg::Source) -> dto::Jpeg {
    let m = s.metadata();
    let (w, h) = s.dimensions();
    dto::Jpeg {
        width: w,
        height: h,
        component_depth: s.precision(),
        components: s
            .components()
            .iter()
            .map(|c| dto::Component {
                id: c.id,
                horizontal: c.horizontal,
                vertical: c.vertical,
                quantization_table: c.quantization_table,
            })
            .collect(),
        model: match s.model() {
            jpeg::Model::Gray => "gray",
            jpeg::Model::Rgb => "rgb",
            jpeg::Model::YCbCr => "ycbcr",
            jpeg::Model::Cmyk {
                adobe_inverted: true,
            } => "adobe-inverted-cmyk",
            jpeg::Model::Cmyk {
                adobe_inverted: false,
            } => "cmyk",
            jpeg::Model::Ycck => "ycck",
        },
        model_origin: match s.model_origin() {
            jpeg::ModelOrigin::Jfif => "jfif",
            jpeg::ModelOrigin::Adobe => "adobe",
            jpeg::ModelOrigin::ComponentIds => "component-ids",
            jpeg::ModelOrigin::ComponentCountConvention => "component-count-convention",
        },
        frame: match s.frame() {
            jpeg::Frame::Baseline => "baseline",
            jpeg::Frame::ExtendedSequential => "extended-sequential",
            jpeg::Frame::Progressive => "progressive",
        },
        orientation: s.orientation(),
        adobe_transform: s.adobe_transform(),
        density: s.density().map(|d| dto::Density {
            units: d.units,
            x: d.x,
            y: d.y,
        }),
        metadata: dto::Metadata {
            bytes: m.bytes,
            icc_bytes: m.icc_bytes,
            icc_segments: m.icc_segments,
            icc_sha256: (m.icc_segments > 0).then(|| hex(&m.icc_sha256)),
            icc_interpretation: "raw segmented payload identity only; no CMS",
            exif_bytes: m.exif_bytes,
            xmp_bytes: m.xmp_bytes,
            iptc_bytes: m.iptc_bytes,
            iptc_segments: m.iptc_segments,
            comment_bytes: m.comment_bytes,
            comment_segments: m.comment_segments,
            other_bytes: m.other_bytes,
            other_segments: m.other_segments,
            jfif_thumbnail_bytes: m.jfif_thumbnail_bytes,
            trailing_bytes: m.trailing_bytes,
        },
    }
}
fn core(value: impl serde::Serialize) -> Result<Value> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|e| Error::new(ErrorCode::InvariantViolation, e.to_string()))?;
    paintcraft_core::types::json::parse(&bytes)
        .map_err(|e| Error::new(ErrorCode::InvariantViolation, e.to_string()))
}
pub(super) fn report(source: &Source, limits: Limits) -> Result<Value> {
    let (width, height) = source.proposed_dimensions();
    let (orientation_policy, identity_profile, depth_policy) = match source {
        Source::Jpeg(_) => (
            jpeg::NORMALIZATION_POLICY,
            Some(jpeg::IDENTITY_PROFILE),
            "preserve-u8",
        ),
        Source::Png(s) => (
            "as-stored; eXIf is historical metadata",
            Some(paintcraft_formats::image::png_retention::IDENTITY_PROFILE),
            if s.bit_depth == 16 {
                "strip16-high-byte"
            } else {
                "expand-to-rgba8"
            },
        ),
        Source::Bmp(_) => (
            "stored bitmap rows normalized to top-down; no EXIF transform",
            Some(paintcraft_formats::image::bmp_retention::IDENTITY_PROFILE),
            "expand-to-rgba8",
        ),
    };
    core(dto::Report {
        schema: dto::Version::V1,
        source: dto::Digest {
            bytes: source.source_bytes(),
            sha256: hex(&source.source_sha256()),
        },
        inspection_only: true,
        pixel_decode_performed: false,
        authorizes_adoption: false,
        metadata_retained_in_document: false,
        color_management_performed: false,
        limits: dto::Limits {
            input_bytes: limits.input_limit(),
            pixels: limits.pixel_limit(),
            metadata_bytes: limits.effective_metadata_limit(source),
            metadata_scope: "adapter-specific metadata/ICC allowance, excluding fixed descriptor overhead; not total RSS",
        },
        facts: match source {
            Source::Png(s) => dto::Facts::Png {
                facts: Box::new(png(s)),
            },
            Source::Bmp(s) => dto::Facts::Bmp {
                facts: Box::new(bmp(s)),
            },
            Source::Jpeg(s) => dto::Facts::Jpeg {
                facts: Box::new(jpeg(s)),
            },
        },
        proposed_conversion: dto::Proposal {
            width,
            height,
            output_model: "straight-rgba8",
            orientation_policy,
            identity_profile,
            source_attachment_policy: match source {
                Source::Png(source) if source.icc.is_some() || source.resolution.is_some() => {
                    Some(dto::SourceAttachmentPolicy {
                        icc_profile_bytes: source.icc.is_some(),
                        resolution_values: source.resolution.is_some(),
                        color_management_applied: false,
                        active_print_settings: false,
                        requires_attachment_admission: true,
                        requires_final_import_validation: true,
                    })
                }
                Source::Bmp(source) if source.icc.is_some() || source.resolution_raw != [0, 0] => {
                    Some(dto::SourceAttachmentPolicy {
                        icc_profile_bytes: source.icc.is_some(),
                        resolution_values: source.resolution_raw != [0, 0],
                        color_management_applied: false,
                        active_print_settings: false,
                        requires_attachment_admission: true,
                        requires_final_import_validation: true,
                    })
                }
                _ => None,
            },
            depth_policy,
            losses: source.proposed_losses(),
            losses_are_header_derived: true,
            requires_pixel_validation: true,
        },
    })
}
pub(super) fn schema() -> Result<Value> {
    core(schemars::schema_for!(dto::Report))
}
