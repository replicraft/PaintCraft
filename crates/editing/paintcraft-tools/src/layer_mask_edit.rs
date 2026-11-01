//! Captured source-preserving layer-mask intent; no selection or document owner.
use paintcraft_commands::LayerMaskOp;
use uuid::Uuid;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerMaskEdit {
    pub owner: Uuid,
    pub state_id: Uuid,
    pub publication_epoch: u64,
    pub operation: LayerMaskOp,
}
impl LayerMaskEdit {
    pub fn matches_base(&self, owner: Uuid, state_id: Uuid, publication_epoch: u64) -> bool {
        (self.owner, self.state_id, self.publication_epoch) == (owner, state_id, publication_epoch)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_owner_ticket_component_is_required_and_layer_reference_is_stable() {
        let e = LayerMaskEdit {
            owner: Uuid::from_u128(1),
            state_id: Uuid::from_u128(2),
            publication_epoch: 3,
            operation: LayerMaskOp::Remove {
                layer: paintcraft_commands::LayerRef::Id(Uuid::from_u128(4)),
            },
        };
        assert!(e.matches_base(e.owner, e.state_id, 3));
        assert!(!e.matches_base(Uuid::from_u128(9), e.state_id, 3));
        assert!(!e.matches_base(e.owner, Uuid::from_u128(9), 3));
        assert!(!e.matches_base(e.owner, e.state_id, 4));
        assert_eq!(e.operation.stable_uuid_ref(), Some(Uuid::from_u128(4)));
    }
}
