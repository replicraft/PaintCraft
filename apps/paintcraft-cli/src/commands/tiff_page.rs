//! One selected TIFF page becomes a reviewed RGBA8 project, never a TIFF save.
use super::*;
use paintcraft_formats::tiff_pages::{self, Assessment, Components, Descriptor};

pub(super) struct Converted {
    pub document: Document,
    pub losses: Vec<String>,
    pub assessment: Value,
}

pub(super) fn convert(
    bytes: &[u8],
    page: u16,
    source_sha: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    let decoded = tiff_pages::decode_page(bytes, page, image::DECODE_PIXELS_MAX, check)?;
    if hex(&decoded.assessment.source_sha256()) != source_sha {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "TIFF page result differs from the captured source",
        ));
    }
    // A different page or conversion policy must not reuse the same state ID.
    // Raw file identity remains separate in the assessment and source guard.
    let identity = hex(&decoded.assessment.document_identity());
    let document = paintcraft_document::import_image_checked(
        decoded.width,
        decoded.height,
        &decoded.rgba,
        &identity,
        check,
    )?;
    check()?;
    Ok(Converted {
        document,
        assessment: assessment_json(&decoded.assessment),
        losses: decoded.losses,
    })
}

fn components(value: Components) -> Value {
    Value::object([
        ("count", Value::Int(i64::from(value.count))),
        (
            "values",
            Value::Array(
                value
                    .values
                    .into_iter()
                    .map(|v| Value::Int(i64::from(v)))
                    .collect(),
            ),
        ),
    ])
}
fn rational(value: Option<(u32, u32)>) -> Value {
    value.map_or(Value::Null, |(n, d)| {
        Value::Array(vec![Value::Int(i64::from(n)), Value::Int(i64::from(d))])
    })
}
fn descriptor(value: &Descriptor) -> Value {
    Value::object([
        ("ifd_offset", Value::Int(value.ifd_offset as i64)),
        ("little_endian", Value::Bool(value.little_endian)),
        ("storage", Value::Int(i64::from(value.storage))),
        ("bits_per_sample", components(value.bits_per_sample)),
        ("sample_format", components(value.sample_format)),
        ("extra_samples", components(value.extra_samples)),
        ("photometric", Value::Int(i64::from(value.photometric))),
        (
            "samples_per_pixel",
            Value::Int(i64::from(value.samples_per_pixel)),
        ),
        ("compression", Value::Int(i64::from(value.compression))),
        ("orientation", Value::Int(i64::from(value.orientation))),
        (
            "planar_configuration",
            Value::Int(i64::from(value.planar_configuration)),
        ),
        ("icc_bytes", Value::Int(i64::from(value.icc_bytes))),
        (
            "icc_sha256",
            if value.icc_bytes == 0 {
                Value::Null
            } else {
                Value::str(hex(&value.icc_sha256))
            },
        ),
        (
            "metadata_bytes",
            Value::Int(i64::from(value.metadata_bytes)),
        ),
        ("omitted_metadata", Value::Bool(value.omitted_metadata)),
        ("resolution_x", rational(value.resolution_x)),
        ("resolution_y", rational(value.resolution_y)),
        (
            "resolution_unit",
            value
                .resolution_unit
                .map_or(Value::Null, |v| Value::Int(i64::from(v))),
        ),
    ])
}
fn assessment_json(value: &Assessment) -> Value {
    Value::object([
        ("policy", Value::str(value.policy())),
        ("profile_variant", Value::str(tiff_pages::IMPORT_VARIANT)),
        ("source_sha256", Value::str(hex(&value.source_sha256()))),
        ("source_bytes", Value::Int(value.source_bytes() as i64)),
        ("page_index", Value::Int(i64::from(value.page_index()))),
        ("page_count", Value::Int(i64::from(value.page_count()))),
        ("width", Value::Int(i64::from(value.width()))),
        ("height", Value::Int(i64::from(value.height()))),
        ("source_depth", Value::Int(i64::from(value.source_depth()))),
        ("output_depth", Value::Int(8)),
        ("output_sha256", Value::str(hex(&value.output_sha256()))),
        ("assessment_sha256", Value::str(hex(&value.digest()))),
        (
            "document_identity",
            Value::str(hex(&value.document_identity())),
        ),
        ("descriptor", descriptor(value.descriptor())),
        (
            "losses",
            Value::Array(value.losses().iter().map(|s| Value::str(*s)).collect()),
        ),
    ])
}
