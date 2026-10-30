//! Immutable fitted previews; encode exactly these buffers after explicit consent.
use super::*;
use paintcraft_document::Document;

#[derive(Debug)]
pub struct PreparedEntry {
    pub kind: IconType,
    rgba: Vec<u8>,
}
impl PreparedEntry {
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
#[derive(Debug)]
pub struct Prepared {
    entries: Vec<PreparedEntry>,
    losses: Vec<String>,
    metadata_omitted: bool,
    filters_baked: bool,
}
impl Prepared {
    pub fn entries(&self) -> &[PreparedEntry] {
        &self.entries
    }
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    pub fn encode(&self, allowed: &[String], check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
        crate::icons::check_export_loss_codes(allowed)?;
        if let Some(loss) = self.losses.iter().find(|loss| {
            !allowed.contains(loss)
                && !(self.metadata_omitted
                    && loss.as_str() == crate::export::metadata_policy::OMITTED_REASON)
                && !(self.filters_baked
                    && loss.as_str() == crate::export::filter_bake::BAKED_REASON)
        }) {
            return Err(unsupported(&format!(
                "ICNS export requires acknowledgment of {loss}"
            )));
        }
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|entry| WriteEntry {
                kind: entry.kind,
                rgba: entry.rgba(),
            })
            .collect();
        encode_entries(&entries, check)
    }
}
/// Fit each explicit qualified PNG type independently from the original canvas.
/// Preserves logical type/scale and owns every preview buffer. Source and aggregate
/// output each admit at most 4,194,304 pixels; shared ICO/CUR resampling semantics apply.
pub fn prepare_document(
    doc: &Document,
    kinds: &[IconType],
    filter: crate::icons::Filter,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    prepare_document_with_metadata_policy(
        doc,
        kinds,
        filter,
        check,
        crate::export::metadata_policy::Policy::default(),
    )
}
pub fn prepare_document_with_metadata_policy(
    doc: &Document,
    kinds: &[IconType],
    filter: crate::icons::Filter,
    check: &dyn Fn() -> Result<()>,
    policy: crate::export::metadata_policy::Policy,
) -> Result<Prepared> {
    prepare_document_with_policies(
        doc,
        kinds,
        filter,
        check,
        crate::export::CopyPolicies {
            source_metadata: policy,
            ..Default::default()
        },
    )
}
pub fn prepare_document_with_policies(
    doc: &Document,
    kinds: &[IconType],
    filter: crate::icons::Filter,
    check: &dyn Fn() -> Result<()>,
    policies: crate::export::CopyPolicies,
) -> Result<Prepared> {
    check()?;
    let target = crate::export::metadata_policy::Target::new(
        crate::registry::Family::Icns,
        crate::registry::Operation::ExportAsset,
    );
    let metadata_omitted = policies.source_metadata.validate(doc, target)?;
    if kinds.is_empty() || kinds.len() > IconType::WRITABLE.len() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "ICNS output needs 1..8 distinct qualified PNG types",
        ));
    }
    for (index, kind) in kinds.iter().enumerate() {
        if !kind.writable() {
            return Err(unsupported("ICNS requested output type is unqualified"));
        }
        if kinds[..index].contains(kind) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "duplicate ICNS output type",
            ));
        }
    }
    crate::icons::admit_source(doc)?;
    let filters_baked = policies.filter_bake.validate(doc, target, check)?;
    let sizes: Vec<_> = kinds
        .iter()
        .map(|kind| {
            let side = kind.pixel_size();
            (side, side)
        })
        .collect();
    let (buffers, mut losses) = crate::icons::prepare_pixels(doc, &sizes, filter, check)?;
    if metadata_omitted {
        losses.push(crate::export::metadata_policy::OMITTED_REASON.to_owned());
    }
    if filters_baked {
        losses.push(crate::export::filter_bake::BAKED_REASON.to_owned());
    }
    let entries = kinds
        .iter()
        .copied()
        .zip(buffers)
        .map(|(kind, rgba)| PreparedEntry { kind, rgba })
        .collect();
    check()?;
    Ok(Prepared {
        entries,
        losses,
        metadata_omitted,
        filters_baked,
    })
}
