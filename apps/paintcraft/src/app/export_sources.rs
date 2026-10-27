//! Retain acquired input identities throughout preparation, picking and publication.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetLease};
use paintcraft_platform::source::SourceIdentity;

#[derive(Debug)]
pub(crate) struct Sources {
    paths: Vec<PathBuf>,
    identities: Vec<SourceIdentity>,
    _metadata: BudgetLease,
}
impl Sources {
    pub(crate) fn capture(app: &App, budget: &paintcraft_runtime::SharedBudget) -> Result<Self> {
        let paths: Vec<_> = app
            .document_source_paths()
            .into_iter()
            .chain(app.asset_source_paths())
            .collect();
        let auxiliary = app.auxiliary_source_guards();
        let retained: Vec<_> = app
            .palettes
            .source_identity()
            .into_iter()
            .chain(app.shapes.source_identity())
            .chain(app.shapes.pending_source_identity())
            .chain(app.brushes.source_identity())
            .chain(auxiliary.iter().map(|source| &source.identity))
            .collect();
        let state_bytes = (app.path_text.len() as u64)
            .checked_add(
                app.bound
                    .as_deref()
                    .map_or(0, |path| path.as_os_str().as_encoded_bytes().len() as u64),
            )
            .and_then(|sum| {
                sum.checked_add(
                    app.editor
                        .path()
                        .map_or(0, |path| path.as_os_str().as_encoded_bytes().len() as u64),
                )
            })
            .and_then(|sum| sum.checked_add(4096))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Image export state metadata overflow",
                )
            })?;
        let identity_bytes = (retained.len() as u64).checked_mul(512).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Image export identity metadata overflow",
            )
        })?;
        let metadata = paths
            .iter()
            .try_fold(state_bytes, |sum, path| {
                sum.checked_add(path.as_os_str().as_encoded_bytes().len() as u64)
                    .and_then(|sum| sum.checked_add(512))
                    .ok_or_else(|| {
                        Error::new(
                            ErrorCode::ResourceLimit,
                            "Image export source metadata overflow",
                        )
                    })
            })?
            .checked_add(identity_bytes)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Image export source identity overflow",
                )
            })?;
        let lease = budget.reserve(metadata)?;
        let mut identities: Vec<_> = retained.into_iter().cloned().collect();
        for path in &paths {
            if let Some(identity) = SourceIdentity::existing_destination(path)? {
                identities.push(identity);
            }
        }
        Ok(Self {
            paths: paths.into_iter().map(Path::to_path_buf).collect(),
            identities,
            _metadata: lease,
        })
    }
    pub(crate) fn check_destination(&self, destination: &Path) -> Result<()> {
        let candidate = SourceIdentity::existing_destination(destination)?;
        if self
            .paths
            .iter()
            .any(|path| crate::tabs::same_destination(path, destination))
            || candidate.is_some_and(|candidate| {
                self.identities
                    .iter()
                    .any(|identity| identity.matches(&candidate))
            })
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "an exported file must not replace an acquired source",
            ));
        }
        Ok(())
    }
}
