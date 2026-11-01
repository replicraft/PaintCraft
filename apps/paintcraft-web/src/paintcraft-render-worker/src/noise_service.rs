//! Source6 is a separate service/type, not a new field or widening of old Service.
use crate::*;
use paintcraft_core::types::resources::BudgetAccount;
use paintcraft_formats::export::prepared_capabilities::{
    self as capabilities, RegistryContract, Requirements, ResolvedRequirements,
};
use paintcraft_render::noise_work::WorkComponents;
use protocol::scoped_programs as wire;
mod begin;
mod copy;
mod frame;
mod open;
mod owner;
mod policy;
mod quote;
use owner::Shared;
pub use policy::SlicePolicy;
/// Separate data allocation. Its whole inline frame/owner layout is priced before
/// Box allocation. Large owned frame buffers carry their own ordered accounts.
#[derive(Debug)]
struct State {
    source: Source,
    base: Document,
    candidate: Option<Document>,
    frame: Option<frame::NoiseFrame>,
    last_binding: Option<Value>,
    generation: Option<u64>,
    profile: wire::Profile,
    required: ResolvedRequirements,
    policy: SlicePolicy,
    workspace: BudgetScope,
    output: BudgetScope,
    _controls_scope: BudgetScope,
}
/// New opaque handle, never cloned. Caller/bindgen handle bookkeeping is reported
/// separately; its retained State allocation and shared owners are charged here.
#[derive(Debug)]
pub struct NoiseService {
    state: Box<State>,
    // Source data/box drop first. Any escaped CompletedFrame retains BOTH owners.
    baseline: Shared<BudgetLease>,
    controls: Shared<BudgetAccount>,
}
#[derive(Clone, Copy, Debug)]
pub struct ControlQuote {
    pub retained_bytes: u64,
    pub handle_bytes: u64,
    pub frame_bytes: u64,
    pub extra_output_bytes: u64,
}
/// Native consumers cannot separate pixels from their actual-capacity declaration.
#[derive(Debug)]
pub struct CompletedFrame {
    binding: Value,
    pixels: Vec<u8>,
    _output: BudgetAccount,
    _baseline: Shared<BudgetLease>,
    _controls: Shared<BudgetAccount>,
}
impl CompletedFrame {
    pub fn binding(&self) -> &Value {
        &self.binding
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}
impl NoiseService {
    pub fn control_quote() -> Result<ControlQuote> {
        let baseline_owner = owner::arc_bytes::<BudgetLease>()?;
        let charge_owner = owner::arc_bytes::<BudgetAccount>()?;
        let retained_bytes = (std::mem::size_of::<State>() as u64)
            .checked_add(baseline_owner)
            .and_then(|n| n.checked_add(charge_owner))
            .ok_or_else(|| resource("Source6 retained control quote"))?;
        Ok(ControlQuote {
            retained_bytes,
            handle_bytes: std::mem::size_of::<Self>() as u64,
            frame_bytes: std::mem::size_of::<frame::NoiseFrame>() as u64,
            extra_output_bytes: OUTPUT_PIXELS_MAX * 4,
        })
    }
    /// No default slice tuple: trusted endpoint supplies the later qualified policy.
    pub fn open(bytes: &[u8], control: &str, policy: SlicePolicy) -> Result<Self> {
        Self::open_checked(bytes, control, policy, &|| Ok(()))
    }
    pub fn open_checked(
        bytes: &[u8],
        control: &str,
        policy: SlicePolicy,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        open::open(bytes, control, policy, check)
    }
    pub fn capabilities(control: &str, check: &dyn Fn() -> Result<()>) -> Result<Value> {
        wire::hello(control, check)
    }
    pub fn context(&self) -> Value {
        if let Some(binding) = &self.state.last_binding {
            return binding.clone();
        }
        let mut value = self
            .state
            .source
            .value
            .as_object()
            .expect("validated Source6")
            .clone();
        value.insert(
            "candidateRequirements".into(),
            self.state.profile.source.value(),
        );
        value.insert(
            "unionRequirements".into(),
            self.state.profile.source.value(),
        );
        value.insert(
            "workloadSha256".into(),
            Value::str(hex(&self
                .state
                .profile
                .workload
                .digest()
                .expect("validated workload"))),
        );
        Value::Object(value)
    }
    pub fn prepare(&mut self, control: &str) -> Result<Value> {
        self.prepare_checked(control, &|| Ok(()))
    }
    pub fn prepare_checked(
        &mut self,
        control: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Value> {
        begin::prepare(self, control, check)
    }
    pub fn step(&mut self) -> Result<bool> {
        self.step_checked(&|| Ok(()))
    }
    pub fn step_checked(&mut self, check: &dyn Fn() -> Result<()>) -> Result<bool> {
        let doc = self.state.candidate.as_ref().unwrap_or(&self.state.base);
        let result = self
            .state
            .frame
            .as_mut()
            .ok_or_else(|| bad("No Source6 frame"))?
            .step(doc, check);
        if result.is_err() {
            self.retire();
        }
        result
    }
    fn ensure_no_escape(&self) -> Result<()> {
        if self.baseline.strong_count() != 1 || self.controls.strong_count() != 1 {
            return Err(resource(
                "Release the owned Source6 frame before another preparation or copy",
            ));
        }
        Ok(())
    }
    pub fn take_frame(&mut self) -> Result<CompletedFrame> {
        self.ensure_no_escape()?;
        if !self
            .state
            .frame
            .as_ref()
            .is_some_and(|f| f.complete() && !f.delivered)
        {
            return Err(bad("Source6 frame incomplete"));
        }
        let f = self.state.frame.take().expect("complete frame");
        Ok(CompletedFrame {
            binding: f.binding,
            pixels: f.pixels,
            _output: f.output,
            _baseline: self.baseline.clone(),
            _controls: self.controls.clone(),
        })
    }
    /// Only synchronous bindgen conversion may use this temporary extra slot.
    /// The source remains alive through conversion; native callers use the owned
    /// CompletedFrame instead. The two Rust slots and JS copy are pre-admitted.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn copy_frame_for_wasm(&mut self) -> Result<Vec<u8>> {
        self.ensure_no_escape()?;
        let f = self
            .state
            .frame
            .as_mut()
            .ok_or_else(|| bad("No Source6 frame"))?;
        if !f.complete() || f.delivered {
            return Err(bad("Source6 frame incomplete or consumed"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(f.pixels.len())
            .map_err(|_| resource("Source6 transfer allocation"))?;
        if bytes.capacity() as u64 > OUTPUT_PIXELS_MAX * 4 {
            return Err(resource("Source6 transfer capacity"));
        }
        bytes.extend_from_slice(&f.pixels);
        f.delivered = true;
        Ok(bytes)
    }
    pub fn cancel(&mut self, generation: u64) -> Result<()> {
        if self.state.generation != Some(generation) {
            return Err(stale());
        }
        self.retire();
        Ok(())
    }
    pub fn retire(&mut self) {
        self.state.frame = None;
        self.state.candidate = None;
        self.state.last_binding = None;
    }
}
#[cfg(test)]
mod tests;
