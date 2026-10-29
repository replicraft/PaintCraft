use super::*;
use paintcraft_core::types::{Error, ErrorCode, json, policy::hex, resources::SharedBudget};
use paintcraft_document::source_metadata::{
    BYTES_MAX, SourceColorModel, SourceFormat, WorkingColor,
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, path::Path};
mod fixtures;
mod guards;
mod persistence;

const PIXELS: u64 = 65_536;
fn fixture(folder: &str, name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(folder)
            .join(name),
    )
    .unwrap()
}
fn own(name: &str) -> Vec<u8> {
    fixture("bmp-retention-fixtures", name)
}
fn prepared(
    bytes: &[u8],
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    decode(
        bytes,
        PIXELS,
        crate::pcnd::MetadataAdmission {
            bytes: BYTES_MAX,
            budget,
        },
        check,
    )
}
fn document(prepared: &Prepared) -> Result<paintcraft_document::Document> {
    prepared.document_with_admission(&mut paintcraft_raster::Admission::new(PIXELS), &|| Ok(()))
}
fn numbers(row: &json::Value, field: &str) -> Vec<u8> {
    row.get(field)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as u8)
        .collect()
}
fn original_profile(bytes: &[u8]) -> &[u8] {
    let start = 14 + u32::from_le_bytes(bytes[126..130].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[130..134].try_into().unwrap()) as usize;
    &bytes[start..start + count]
}
fn with_profile_size(size: usize) -> Vec<u8> {
    let original = own("rgb24-v124-profile-positive.bmp");
    let start = 14 + u32::from_le_bytes(original[126..130].try_into().unwrap()) as usize;
    let mut bytes = original[..start].to_vec();
    let mut profile = original_profile(&original).to_vec();
    assert!(size >= profile.len());
    profile.resize(size, 0);
    profile[..4].copy_from_slice(&(size as u32).to_be_bytes());
    bytes.extend(profile);
    let length = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&length.to_le_bytes());
    bytes[130..134].copy_from_slice(&(size as u32).to_le_bytes());
    bytes
}
