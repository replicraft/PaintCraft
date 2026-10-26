//! Bounded entry discovery without decoding pixels or publishing a document.
use super::*;
use paintcraft_formats::{
    icns,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
}
fn encoding(value: icns::Encoding) -> &'static str {
    match value {
        icns::Encoding::Png => "png",
        icns::Encoding::LegacyRaw => "rgb-mask-raw",
        icns::Encoding::LegacyRle => "rgb-mask-rle",
        icns::Encoding::Unqualified => "unqualified",
    }
}
pub(super) fn inspect_path(
    path: &Path,
    frontend: Frontend,
    expected: Option<&str>,
) -> Result<(Value, String)> {
    let mut request = Request::new(
        frontend,
        Operation::ImportConvert,
        Family::Icns,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|value| value.to_str());
    if request.extension.is_none() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "ICNS inspection requires .icns extension",
        ));
    }
    request.variant = Some("icon-selected-entry");
    registry::admission(request).require_allowed(request)?;
    let (bytes, sha, _identity) =
        read_captured(path, icns::BYTES_MAX as u64, "ICNS inspection source")?;
    guard(expected, &sha)?;
    let directory = icns::inspect(&bytes, &|| Ok(()))?;
    let mut text = format!(
        "ICNS: {} ordered entries, sha256 {sha}",
        directory.entries.len()
    );
    let entries = directory
        .entries
        .iter()
        .map(|entry| {
            let tag = String::from_utf8_lossy(&entry.tag);
            text.push_str(&format!(
                "\n{}: {tag}, {}x{} logical at {}x, {}x{} pixels, {}",
                entry.index,
                entry.logical_width,
                entry.logical_height,
                entry.scale,
                entry.width,
                entry.height,
                encoding(entry.encoding)
            ));
            Value::object([
                ("index", Value::Int(i64::from(entry.index))),
                ("record", Value::Int(i64::from(entry.record))),
                ("tag", Value::str(tag)),
                ("logical_width", Value::Int(i64::from(entry.logical_width))),
                (
                    "logical_height",
                    Value::Int(i64::from(entry.logical_height)),
                ),
                ("scale", Value::Int(i64::from(entry.scale))),
                ("width", Value::Int(i64::from(entry.width))),
                ("height", Value::Int(i64::from(entry.height))),
                ("encoding", Value::str(encoding(entry.encoding))),
                (
                    "qualified_for_import",
                    Value::Bool(entry.encoding != icns::Encoding::Unqualified),
                ),
            ])
        })
        .collect();
    let records = directory
        .records
        .iter()
        .map(|record| {
            Value::object([
                ("tag", Value::str(String::from_utf8_lossy(&record.tag))),
                ("offset", Value::Int(record.offset as i64)),
                ("length", Value::Int(record.length as i64)),
            ])
        })
        .collect();
    Ok((
        Value::object([
            ("format", Value::str("icns")),
            ("source_sha256", Value::str(sha)),
            (
                "encoded_length",
                Value::Int(directory.encoded_length as i64),
            ),
            ("assessment_only", Value::Bool(true)),
            ("preserving_save", Value::Bool(false)),
            ("entries", Value::Array(entries)),
            ("records", Value::Array(records)),
        ]),
        text,
    ))
}
pub fn run(a: Options, json: bool) -> Reply {
    bare(
        json,
        inspect_path(
            &a.input,
            a.profile.frontend.into(),
            a.expect_input_sha256.as_deref(),
        ),
    )
}
