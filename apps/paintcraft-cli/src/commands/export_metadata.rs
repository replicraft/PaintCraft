//! Explicit copy intent shared by the specialty command adapters.
//! Backends independently verify the hardcoded family and copy operation.
use super::*;
use paintcraft_formats::{
    export::metadata_policy,
    icons,
    registry::{Family, Operation},
};

#[derive(Debug, Default, clap::Args)]
#[group(id = "SourceMetadataCopyOptions")]
pub(super) struct Options {
    /// Retained source metadata: refuse (default), or explicitly omit from this foreign copy.
    #[arg(long = "source-metadata", default_value = "refuse")]
    pub intent: metadata_policy::Intent,
}
/// Match the document backend's actual preparation operation. The registry's
/// SaveAsCopy entry point is admission, not a different metadata witness target.
#[derive(Clone, Copy, Debug)]
pub(super) enum CopyFormat {
    Icon(icons::Kind),
    Icns,
    Dcx,
    Jxr,
}
impl CopyFormat {
    fn target(self) -> metadata_policy::Target {
        let (family, operation) = match self {
            Self::Icon(icons::Kind::Icon) => (Family::Ico, Operation::ExportAsset),
            Self::Icon(icons::Kind::Cursor) => (Family::Cur, Operation::ExportAsset),
            Self::Icns => (Family::Icns, Operation::ExportAsset),
            Self::Dcx => (Family::Dcx, Operation::ExportFlat),
            Self::Jxr => (Family::Jxr, Operation::ExportFlat),
        };
        metadata_policy::Target::new(family, operation)
    }
    pub(super) fn capture(
        self,
        doc: &Document,
        intent: metadata_policy::Intent,
    ) -> Result<metadata_policy::Policy> {
        metadata_policy::Policy::capture(doc, self.target(), intent)
    }
}
impl Options {
    pub(super) fn capture(
        &self,
        doc: &Document,
        format: CopyFormat,
    ) -> Result<metadata_policy::Policy> {
        format.capture(doc, self.intent)
    }
}

/// Both intents originate in explicit CLI/RPC input, not codec preview losses.
pub(super) fn capture_copy_policies(
    doc: &Document,
    target: metadata_policy::Target,
    metadata: metadata_policy::Intent,
    filters: export::filter_bake::Intent,
    check: &dyn Fn() -> Result<()>,
) -> Result<export::CopyPolicies> {
    Ok(export::CopyPolicies {
        source_metadata: metadata_policy::Policy::capture(doc, target, metadata)?,
        filter_bake: export::filter_bake::Policy::capture(doc, target, filters, check)?,
    })
}
impl Options {
    pub(super) fn capture_policies(
        &self,
        doc: &Document,
        format: CopyFormat,
        filters: export::filter_bake::Intent,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<export::CopyPolicies> {
        Ok(export::CopyPolicies {
            source_metadata: self.capture(doc, format)?,
            filter_bake: export::filter_bake::Policy::capture(
                doc,
                format.target(),
                filters,
                check,
            )?,
        })
    }
}

/// Invoke only at the RPC boundary on its unmodified, explicit user loss list.
pub(super) fn user_filter_intent(losses: &[String]) -> export::filter_bake::Intent {
    if losses
        .iter()
        .any(|loss| loss == export::filter_bake::BAKED_REASON)
    {
        export::filter_bake::Intent::Bake
    } else {
        export::filter_bake::Intent::Refuse
    }
}
impl CopyFormat {
    pub(super) fn capture_policies(
        self,
        doc: &Document,
        metadata: metadata_policy::Intent,
        filters: export::filter_bake::Intent,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<export::CopyPolicies> {
        Ok(export::CopyPolicies {
            source_metadata: self.capture(doc, metadata)?,
            filter_bake: export::filter_bake::Policy::capture(doc, self.target(), filters, check)?,
        })
    }
}
