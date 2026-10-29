//! One captured source and explicit copy policy for native document-based assets.
use super::Frozen;
use crate::App;
use eframe::egui;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::export::metadata_policy::{Intent, OMITTED_REASON, Policy, Target};

pub(crate) struct AssetCopy<C> {
    pub frozen: Frozen,
    pub omit: bool,
    prepared: Option<(C, Target, Policy)>,
}
impl<C: Clone + PartialEq> AssetCopy<C> {
    pub fn capture(app: &App) -> Result<Self> {
        Ok(Self {
            frozen: Frozen::capture(app)?,
            omit: false,
            prepared: None,
        })
    }
    pub fn current(&self, app: &App) -> bool {
        self.frozen.current(app)
    }
    pub fn ready(&self) -> bool {
        self.frozen.document.source_metadata.is_none() || self.omit
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) -> bool {
        let before = self.omit;
        if self.frozen.document.source_metadata.is_some() {
            paintcraft_ui::source_metadata_export::controls(ui, &mut self.omit);
        }
        let changed = before != self.omit;
        if changed {
            self.retire();
        }
        changed
    }
    pub fn retire(&mut self) {
        self.prepared = None;
    }
    /// Called only while preparing a new cache entry, never after an await.
    pub fn prepare(&mut self, config: C, target: Target) -> Result<Policy> {
        let policy = Policy::capture(
            &self.frozen.document,
            target,
            if self.omit {
                Intent::Omit
            } else {
                Intent::Refuse
            },
        )?;
        policy.validate(&self.frozen.document, target)?;
        self.prepared = Some((config, target, policy));
        Ok(policy)
    }
    /// Validate the original witness and selected configuration at publication.
    pub fn validate(&self, config: &C, target: Target) -> Result<()> {
        let Some((original, original_target, policy)) = &self.prepared else {
            return Err(stale());
        };
        if original != config
            || *original_target != target
            || matches!(policy, Policy::Omit(_)) != self.omit
        {
            return Err(stale());
        }
        policy.validate(&self.frozen.document, target)?;
        Ok(())
    }
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Asset copy policy no longer matches its frozen document and settings",
    )
}
/// The separate metadata checkbox authorizes its own receipt, not other losses.
pub(crate) fn other_losses(losses: &[String]) -> Vec<String> {
    losses
        .iter()
        .filter(|loss| loss.as_str() != OMITTED_REASON)
        .cloned()
        .collect()
}
