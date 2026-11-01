//! Cheap wanted identity; the one preparation ledger starts after old close ACK.
use super::{capture::Capture7, preparation::Preparation7};
use crate::app::filter_editing::render::Capture;
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use std::sync::Arc;
#[derive(Clone, Debug)]
pub(in crate::app::filter_editing) struct Public7 {
    pub ticket: Ticket,
    pub base: Arc<Document>,
}
#[derive(Clone, Debug)]
pub(in crate::app::filter_editing) struct Key7 {
    source: Capture,
    parent: super::parent::Parent7,
    public: Public7,
    operation: Option<FilterOp>,
    pub copy_spec: Option<paintcraft_core::types::json::Value>,
}
impl Key7 {
    pub fn new(
        source: Capture,
        operation: Option<&FilterOp>,
        copy_spec: Option<&paintcraft_core::types::json::Value>,
    ) -> Result<Self> {
        if operation.is_some_and(|op| {
            matches!(op, FilterOp::Mask { .. }) || op.stable_uuid_refs().is_none()
        }) {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source7 numeric preview cannot mutate masks",
            ));
        }
        if operation.is_some_and(|op| matches!(op, FilterOp::Add { alias, .. } | FilterOp::Duplicate { alias, .. } if alias.len() > 64)) {
            return Err(Error::new(ErrorCode::ResourceLimit, "Source7 preview alias bound"));
        }
        let public = source.publication.clone().unwrap_or_else(|| Public7 {
            ticket: source.ticket,
            base: source.document.clone(),
        });
        Ok(Self {
            parent: super::parent::Parent7::capture()?,
            source,
            public,
            operation: operation.cloned(),
            copy_spec: copy_spec.cloned(),
        })
    }
    pub fn same(&self, other: &Self) -> bool {
        self.parent.same(&other.parent)
            && self.source.owner == other.source.owner
            && self.source.draft == other.source.draft
            && self.public.ticket == other.public.ticket
            && Arc::ptr_eq(&self.public.base, &other.public.base)
            && Arc::ptr_eq(&self.source.document, &other.source.document)
            && self.operation == other.operation
            && self.copy_spec == other.copy_spec
    }
    pub fn prepare_capture(
        &self,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Capture7, Preparation7)> {
        let prefix = Preparation7::from_parent(self.parent.clone())?;
        let mut capture = Capture7::capture(
            self.source.owner,
            self.public.ticket,
            self.source.document.clone(),
            self.source.draft,
            self.operation.as_ref(),
            &prefix,
            check,
        )?;
        capture.public_base = self.public.base.clone();
        Ok((capture, prefix))
    }
}
