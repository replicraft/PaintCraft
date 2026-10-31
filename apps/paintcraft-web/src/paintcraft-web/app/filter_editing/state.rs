//! One immutable owner/base and one checked sequence of private parameter edits.
use super::*;
use paintcraft_commands::{FilterOp, FilterRef, LayerRef};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use paintcraft_tools::FilterEdit;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Revision {
    pub draft: Uuid,
    pub generation: u64,
}
#[derive(Debug)]
pub(super) struct Draft {
    pub revision: Revision,
    pub base: FilterEdit,
    pub source: Arc<Document>,
    pub source_spatial: bool,
    pub layer: Uuid,
    pub parameters: Parameters,
    pub apply: Option<Revision>,
    pub prepared: Option<Revision>,
    pub error: Option<Error>,
}
impl Draft {
    pub fn capture(app: &WebApp, operation: FilterOp) -> Result<Self> {
        let ticket = app.editor.session.ticket();
        let layer = app.editor.target.ok_or_else(stale)?;
        let descriptor = match &operation {
            FilterOp::Add { descriptor, .. } | FilterOp::Update { descriptor, .. } => *descriptor,
            _ => return Err(stale()),
        };
        let base = FilterEdit {
            owner: app.editor.session.owner_id(),
            state_id: ticket.state_id,
            publication_epoch: ticket.publication_epoch,
            operation,
        };
        app.editor.filter_operation(&base)?;
        Ok(Self {
            revision: Revision {
                draft: Uuid::new_v4(),
                generation: 1,
            },
            base,
            source: app.editor.session.snapshot(),
            source_spatial: super::render::stored_spatial(app.editor.session.document()) > 0,
            layer,
            parameters: Parameters::from_descriptor(descriptor),
            apply: None,
            prepared: None,
            error: None,
        })
    }
    pub fn current(&self, app: &WebApp) -> bool {
        let ticket = app.editor.session.ticket();
        app.frontend == Frontend::Studio
            && app.editor.target == Some(self.layer)
            && self.base.matches_base(
                app.editor.session.owner_id(),
                ticket.state_id,
                ticket.publication_epoch,
            )
            && Arc::ptr_eq(&self.source, &app.editor.session.snapshot())
            && !app.filter_private_input()
    }
    pub fn operation(&self) -> Result<FilterEdit> {
        let descriptor = self.parameters.descriptor()?;
        let mut edit = self.base.clone();
        match &mut edit.operation {
            FilterOp::Add {
                descriptor: value, ..
            }
            | FilterOp::Update {
                descriptor: value, ..
            } => *value = descriptor,
            _ => return Err(stale()),
        }
        Ok(edit)
    }
    pub fn change(&mut self) -> Result<()> {
        self.revision.generation = self.revision.generation.checked_add(1).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Filter preview generation exhausted",
            )
        })?;
        self.apply = None;
        self.prepared = None;
        self.error = None;
        Ok(())
    }
    pub fn ticket(&self) -> Ticket {
        Ticket {
            state_id: self.base.state_id,
            publication_epoch: self.base.publication_epoch,
        }
    }
}
pub(super) fn stale() -> Error {
    Error::new(ErrorCode::ConflictStaleBase, "Filter edit source changed")
}
pub(super) fn operation(app: &WebApp, intent: Intent<Uuid>) -> Option<FilterOp> {
    let id = app.editor.target?;
    let source = app.editor.session.document().layer(&id)?;
    let layer = LayerRef::Id(id);
    let reference = |id| {
        source
            .filters
            .iter()
            .find(|f| f.id() == id)
            .map(|_| FilterRef::Id(id))
    };
    Some(match intent {
        Intent::Add(kind) if source.filters.len() < 256 => FilterOp::Add {
            layer,
            alias: "studio_filter".into(),
            descriptor: Parameters::new(kind).descriptor().ok()?,
            enabled: true,
            to_index: None,
        },
        Intent::Edit(id) => FilterOp::Update {
            layer,
            filter: reference(id)?,
            descriptor: source.filters.iter().find(|f| f.id() == id)?.descriptor(),
        },
        Intent::SetEnabled { id, enabled } => FilterOp::SetEnabled {
            layer,
            filter: reference(id)?,
            enabled,
        },
        Intent::Reorder { id, to_index } if to_index < source.filters.len() => FilterOp::Reorder {
            layer,
            filter: reference(id)?,
            to_index,
        },
        Intent::Duplicate(id) if source.filters.len() < 256 => FilterOp::Duplicate {
            layer,
            filter: reference(id)?,
            alias: "studio_filter".into(),
            to_index: None,
        },
        Intent::UseSelection(id) => FilterOp::Mask {
            layer,
            filter: reference(id)?,
            operation: paintcraft_commands::MaskOp::FromSelection { enabled: true },
        },
        Intent::SetMaskEnabled { id, enabled } => FilterOp::Mask {
            layer,
            filter: reference(id)?,
            operation: paintcraft_commands::MaskOp::SetEnabled { enabled },
        },
        Intent::RemoveMask(id) => FilterOp::Mask {
            layer,
            filter: reference(id)?,
            operation: paintcraft_commands::MaskOp::Remove,
        },
        Intent::Remove(id) => FilterOp::Remove {
            layer,
            filter: reference(id)?,
        },
        _ => return None,
    })
}
