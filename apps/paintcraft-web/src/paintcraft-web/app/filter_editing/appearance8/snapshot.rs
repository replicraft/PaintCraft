//! Same genuine counted Source8 bytes, authenticated source facts and W forecast.
use super::{
    capture::{Identity8, Key8},
    denied,
    factory::Factory8,
    operation::Operation8,
};
use paintcraft_core::types::{Result, resources::BudgetAccount};
use paintcraft_formats::pcnd::{self, BlendSourceDemand, CountedBlendSnapshotBytes};
use paintcraft_render::blend::metadata::{
    UnboundRootedFrameWorkspace, quote_unbound_rooted_frame_workspace_checked,
    unbound_rooted_frame_workspace_control_bytes,
};
use paintcraft_web_render_worker::source8::{
    Source8RoleDemand, source8_role_demand_checked, source8_role_query_cost,
};
#[derive(Debug)]
pub(super) struct Snapshot8 {
    bytes: Option<CountedBlendSnapshotBytes>,
    transfer: Option<Transfer8>,
    pub identity: Identity8,
    pub demand: BlendSourceDemand,
    pub workspace: UnboundRootedFrameWorkspace,
    pub roles: Source8RoleDemand,
    _controls: BudgetAccount,
    operation: Operation8,
}
impl Snapshot8 {
    #[cfg(test)]
    pub fn prepare(key: Key8, factory: Factory8, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        Preparation8::new(key, factory, check)?.finish(check)
    }
    fn encode(
        identity: Identity8,
        operation: Operation8,
        controls: BudgetAccount,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let raw = || operation.step(check);
        let spend = |n| operation.spend(n);
        let scope = operation.factory().scope();
        let fact = pcnd::count_portable_blend_snapshot_rooted_checked(
            &identity.document,
            scope,
            &raw,
            &spend,
        )?;
        let (bytes, demand) = fact.encode_with_source_forecast_checked()?;
        drop(fact);
        operation.spend(1024)?;
        if demand.document_id != *identity.document.document_id.as_bytes()
            || demand.state_id != *identity.document.state_id.as_bytes()
            || demand.canvas != [identity.document.canvas.0, identity.document.canvas.1]
            || demand.encoded_bytes != bytes.len() as u64
            || !demand.stored_non_normal
            || demand.pointer_bytes as usize != std::mem::size_of::<usize>()
        {
            return Err(denied());
        }
        let workspace = {
            let mut query = scope.account();
            query.resize(unbound_rooted_frame_workspace_control_bytes())?;
            quote_unbound_rooted_frame_workspace_checked(&identity.document, &raw, &spend)?
        };
        if demand.stored_noise != workspace.requires_noise {
            return Err(denied());
        }
        // BOTH facts are produced privately here from this SAME immutable Doc
        // and counted bytes. The public scalar pair alone is not authentication.
        operation.spend(1024)?;
        let cost = source8_role_query_cost()?;
        let roles = {
            let mut query = scope.account();
            query.resize(cost.control_bytes)?;
            operation.spend(cost.formation_work)?;
            source8_role_demand_checked(&demand, &workspace)?
        };
        if roles.compiled_profile != identity.profile {
            return Err(denied());
        }
        Ok(Self {
            bytes: Some(bytes),
            transfer: None,
            identity,
            demand,
            workspace,
            roles,
            _controls: controls,
            operation,
        })
    }
    pub fn bytes(&self) -> Result<&[u8]> {
        self.bytes.as_ref().map(|b| b.as_bytes()).ok_or_else(denied)
    }
    /// Only a genuine OPENED or acknowledged no-source failure permits this.
    pub fn transfer_pending(&self) -> bool {
        self.transfer.is_some()
    }
    pub fn begin_transfer(&mut self) -> Result<()> {
        if self.transfer.is_some() {
            return Err(denied());
        }
        let n = self.demand.encoded_bytes;
        self.operation.spend(
            n.checked_mul(3)
                .and_then(|n| n.checked_add(1024))
                .ok_or_else(denied)?,
        )?;
        let mut account = self.factory().scope().account();
        account.resize(n)?;
        self.transfer = Some(Transfer8 {
            account: Some(account),
            factory: Some(self.factory().clone()),
            joined: false,
        });
        Ok(())
    }
    pub fn opened(&mut self) {
        drop(self.bytes.take());
        if let Some(mut transfer) = self.transfer.take() {
            transfer.joined = true;
            drop(transfer);
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn spend(&self, n: u64) -> Result<()> {
        self.operation.spend(n)
    }
    pub fn factory(&self) -> &Factory8 {
        self.operation.factory()
    }
}

#[derive(Debug)]
pub(super) struct Preparation8 {
    pub identity: Identity8,
    pub blend: bool,
    controls: BudgetAccount,
    operation: Operation8,
}
impl Preparation8 {
    pub fn new(key: Key8, factory: Factory8, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let operation = Operation8::preparation(key.parent, factory)?;
        operation.spend(81920)?;
        super::controls::validate()?;
        let mut controls = operation.factory().scope().account();
        controls.resize(super::controls::snapshot_bytes()?)?;
        operation.step(check)?;
        let doc = &key.identity.document;
        let n = doc.roots.len();
        operation.spend(64)?;
        if n > paintcraft_document::LAYER_COUNT_MAX || n != doc.layers.len() {
            return Err(denied());
        }
        let roots = n as u64;
        let pairs = roots
            .checked_mul(roots.saturating_sub(1))
            .map(|n| n / 2)
            .ok_or_else(denied)?;
        let work = roots
            .checked_mul(4096)
            .and_then(|n| n.checked_add(pairs.checked_mul(18)?))
            .ok_or_else(denied)?;
        operation.spend(work)?;
        let mut blend = false;
        for (at, id) in doc.roots.iter().enumerate() {
            operation.step(check)?;
            if doc.roots[..at].contains(id) {
                return Err(denied());
            }
            let layer = doc
                .layers
                .get(id)
                .filter(|l| l.id == *id)
                .ok_or_else(denied)?;
            if !layer.styles.is_empty() {
                return Err(denied());
            }
            blend |= layer.blend_mode != paintcraft_core::types::LayerBlendMode::Normal;
        }
        Ok(Self {
            identity: key.identity,
            blend,
            controls,
            operation,
        })
    }
    pub fn finish(self, check: &dyn Fn() -> Result<()>) -> Result<Snapshot8> {
        if !self.blend {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                "Source8 normal source selects its existing route",
            ));
        }
        Snapshot8::encode(self.identity, self.operation, self.controls, check)
    }
}

#[derive(Debug)]
struct Transfer8 {
    account: Option<BudgetAccount>,
    factory: Option<Factory8>,
    joined: bool,
}
impl Drop for Transfer8 {
    fn drop(&mut self) {
        if !self.joined {
            if let Some(factory) = &self.factory {
                factory.quarantine();
            }
            std::mem::forget((self.account.take(), self.factory.take()));
        }
    }
}
