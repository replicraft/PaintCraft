//! Actual public owner and SAME immutable counted source; no borrowed App escape.
use super::{denied, operation::Parent8};
use crate::app::{WebApp, filter_editing::render::View};
use paintcraft_core::types::Result;
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub(super) struct Identity8 {
    pub owner: Uuid,
    pub ticket: Ticket,
    pub document: Arc<Document>,
    pub draft: Uuid,
    pub profile: [u8; 32],
}
#[derive(Debug)]
pub(super) struct Key8 {
    pub identity: Identity8,
    pub parent: Parent8,
}
impl Key8 {
    pub fn capture(app: &WebApp) -> Result<Self> {
        if app.filter_editing.is_some() {
            return Err(denied());
        }
        let document = app.editor.session.snapshot();
        let parent = Parent8::capture()?;
        let profile =
            paintcraft_web_render_worker::blend_service::LayerBlendService::compiled_profile()?;
        Ok(Self {
            identity: Identity8 {
                owner: app.editor.session.owner_id(),
                ticket: app.editor.session.ticket(),
                document,
                draft: Uuid::new_v4(),
                profile,
            },
            parent,
        })
    }
}
impl Identity8 {
    pub fn current(&self, app: &WebApp) -> bool {
        app.filter_editing.is_none()
            && self.owner == app.editor.session.owner_id()
            && self.ticket == app.editor.session.ticket()
            && Arc::ptr_eq(&self.document, &app.editor.session.snapshot())
    }
    pub fn same_source(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.ticket == other.ticket
            && self.draft == other.draft
            && self.profile == other.profile
            && Arc::ptr_eq(&self.document, &other.document)
    }
    pub fn validate_view(&self, view: View) -> Result<()> {
        let actual = View::new(&self.document, view.visible, view.level)?;
        if actual != view {
            return Err(denied());
        }
        Ok(())
    }
}
/// Host refreshes after the complete synchronous App.ui, before JS can resume.
/// No raw pointer or &WebApp is captured by an async/JS closure.
#[derive(Clone)]
pub(super) struct CurrentStamp8 {
    pub owner: Uuid,
    pub ticket: Ticket,
    document: Arc<Document>,
    pub private: bool,
}
impl CurrentStamp8 {
    pub fn read(app: &WebApp) -> Self {
        Self {
            owner: app.editor.session.owner_id(),
            ticket: app.editor.session.ticket(),
            document: app.editor.session.snapshot(),
            private: app.filter_editing.is_some(),
        }
    }
    pub fn matches(&self, identity: &Identity8) -> bool {
        !self.private
            && self.owner == identity.owner
            && self.ticket == identity.ticket
            && Arc::ptr_eq(&self.document, &identity.document)
    }
}

impl std::fmt::Debug for Identity8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainSourceIdentity8")
            .field("owner", &self.owner)
            .field("ticket", &self.ticket)
            .field("document_id", &self.document.document_id)
            .field("state_id", &self.document.state_id)
            .field("draft", &self.draft)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for CurrentStamp8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainCurrentStamp8")
            .field("owner", &self.owner)
            .field("ticket", &self.ticket)
            .field("private", &self.private)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Expected8 {
    pub token: u64,
    pub view: View,
    pub calls: u64,
}
