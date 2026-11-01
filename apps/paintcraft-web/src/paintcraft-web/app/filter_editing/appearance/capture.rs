//! Source7 identities remain distinct from the legacy Source2–6 capture flags.
use super::preparation::Preparation7;
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{Error, ErrorCode, Result, filters::Descriptor};
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use paintcraft_formats::export::layer_appearance::{Admission7, Mode7};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone, Debug)]
pub(in crate::app::filter_editing) struct Capture7 {
    pub owner: Uuid,
    /// Actual committed/public source fences; never rewritten for a private view.
    pub public_ticket: Ticket,
    pub public_base: Arc<Document>,
    /// Authenticated PCND snapshot may be an explicitly staged ordinary preview.
    pub document: Arc<Document>,
    pub draft: Uuid,
    pub admission: Admission7,
    /// Fixed typed intent. A different operation requires close ACK/new admission.
    pub operation: Option<FilterOp>,
}
fn noise(doc: &Document, prefix: &Preparation7, check: &dyn Fn() -> Result<()>) -> Result<bool> {
    let mut found = false;
    for layer in doc.layers.values() {
        check()?;
        prefix.spend(1)?;
        prefix.spend(layer.filters.len() as u64)?;
        found |= layer
            .filters
            .iter()
            .any(|f| matches!(f.descriptor(), Descriptor::AddNoiseRgbV1(_)));
    }
    Ok(found)
}
impl Capture7 {
    pub fn capture(
        owner: Uuid,
        ticket: Ticket,
        document: Arc<Document>,
        draft: Uuid,
        prospective: Option<&FilterOp>,
        prefix: &Preparation7,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        prefix.spend(document.layers.len() as u64)?;
        if !document.has_layer_masks() {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source7 requires stored layer masks",
            ));
        }
        if prospective.is_some_and(|op| {
            matches!(op, FilterOp::Mask { .. }) || op.stable_uuid_refs().is_none()
        }) {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source7 requires one stable numerical filter operation",
            ));
        }
        if prospective.is_some_and(|op| matches!(op, FilterOp::Add { alias, .. } | FilterOp::Duplicate { alias, .. } if alias.len() > 64)) {
            return Err(Error::new(ErrorCode::ResourceLimit, "Source7 operation alias bound"));
        }
        let prospective_noise = prospective.is_some_and(|op| {
            matches!(
                op,
                FilterOp::Add {
                    descriptor: Descriptor::AddNoiseRgbV1(_),
                    ..
                } | FilterOp::Update {
                    descriptor: Descriptor::AddNoiseRgbV1(_),
                    ..
                }
            )
        });
        let mode = if noise(&document, prefix, check)? || prospective_noise {
            Mode7::Noise
        } else {
            Mode7::Base
        };
        Ok(Self {
            owner,
            public_ticket: ticket,
            public_base: document.clone(),
            document,
            draft,
            operation: prospective.cloned(),
            admission: prefix.select_mode(mode)?,
        })
    }
    #[cfg(test)]
    pub fn private_snapshot(
        &self,
        document: Arc<Document>,
        prefix: &Preparation7,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        // A changed snapshot is a new explicit intent/OPEN, with a fresh prefix;
        // never mutate or restage an already-open prepared context.
        let mut capture = Self::capture(
            self.owner,
            self.public_ticket,
            document,
            self.draft,
            self.operation.as_ref(),
            prefix,
            check,
        )?;
        capture.public_base = self.public_base.clone();
        Ok(capture)
    }
    pub fn wire_ticket(&self) -> Ticket {
        Ticket {
            state_id: self.document.state_id,
            publication_epoch: self.public_ticket.publication_epoch,
        }
    }
    #[cfg(test)]
    pub fn same(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.public_ticket == other.public_ticket
            && Arc::ptr_eq(&self.public_base, &other.public_base)
            && Arc::ptr_eq(&self.document, &other.document)
            && self.draft == other.draft
            && self.admission == other.admission
            && self.operation == other.operation
    }
    #[cfg(test)]
    pub fn public_current(&self, owner: Uuid, ticket: Ticket, base: &Arc<Document>) -> bool {
        self.owner == owner && self.public_ticket == ticket && Arc::ptr_eq(&self.public_base, base)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_commands::{FilterRef, LayerRef};
    use paintcraft_core::types::{Rect, resources::SharedBudget};
    use paintcraft_document::LayerMask;
    use paintcraft_raster::coverage_mask::{A8Mask, Limits};
    fn source() -> Arc<Document> {
        let mut doc = paintcraft_document::import_image(1, 1, &[4, 5, 6, 255], "capture7").unwrap();
        let coverage = A8Mask::uniform(
            Rect::from_origin_size(0, 0, 1, 1).unwrap(),
            0,
            &SharedBudget::new(4096),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().mask = Some(LayerMask::new(false, coverage));
        Arc::new(doc)
    }
    #[test]
    fn fixed_typed_intent_distinguishes_new_context_without_replacing_public_fences() {
        let doc = source();
        let ticket = Ticket {
            state_id: doc.state_id,
            publication_epoch: 7,
        };
        let owner = Uuid::from_u128(1);
        let draft = Uuid::from_u128(2);
        let op = FilterOp::Add {
            layer: LayerRef::Id(doc.roots[0]),
            alias: "added".into(),
            descriptor: Descriptor::InvertRgbV1,
            enabled: true,
            to_index: None,
        };
        let original = Capture7::capture(
            owner,
            ticket,
            doc.clone(),
            draft,
            Some(&op),
            &Preparation7::before_capture(),
            &|| Ok(()),
        )
        .unwrap();
        assert!(original.same(&original.clone()));
        let base = Capture7::capture(
            owner,
            ticket,
            doc.clone(),
            draft,
            None,
            &Preparation7::before_capture(),
            &|| Ok(()),
        )
        .unwrap();
        assert!(!original.same(&base));
        let mut changed = original.clone();
        if let Some(FilterOp::Add { enabled, .. }) = &mut changed.operation {
            *enabled = false;
        }
        assert!(!original.same(&changed));
        let mut private = (*doc).clone();
        private.state_id = Uuid::from_u128(3);
        let private = original
            .private_snapshot(Arc::new(private), &Preparation7::before_capture(), &|| {
                Ok(())
            })
            .unwrap();
        assert!(private.public_current(owner, ticket, &doc));
        assert_ne!(private.wire_ticket().state_id, ticket.state_id);
        assert_eq!(private.document.document_id, doc.document_id);
        assert!(!private.same(&original));
    }
    #[test]
    fn mask_mutations_and_unstable_targets_cannot_enter_numeric_open() {
        let doc = source();
        let ticket = Ticket {
            state_id: doc.state_id,
            publication_epoch: 0,
        };
        let mask_operation = FilterOp::Mask {
            layer: LayerRef::Id(doc.roots[0]),
            filter: FilterRef::Id(Uuid::from_u128(9)),
            operation: paintcraft_commands::MaskOp::Remove,
        };
        assert_eq!(
            Capture7::capture(
                Uuid::from_u128(1),
                ticket,
                doc.clone(),
                Uuid::from_u128(2),
                Some(&mask_operation),
                &Preparation7::before_capture(),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        let operation = FilterOp::Remove {
            layer: LayerRef::Alias("later".into()),
            filter: FilterRef::Id(Uuid::from_u128(9)),
        };
        assert_eq!(
            Capture7::capture(
                Uuid::from_u128(1),
                ticket,
                doc,
                Uuid::from_u128(2),
                Some(&operation),
                &Preparation7::before_capture(),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
