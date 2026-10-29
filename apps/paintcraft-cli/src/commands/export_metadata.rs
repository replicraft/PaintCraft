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
