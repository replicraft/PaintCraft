//! Source7 retained source storage plus bounded execution jobs.
use crate::*;
use paintcraft_core::types::resources::BudgetAccount;
use paintcraft_formats::{
    export::{
        layer_appearance::{
            self as metadata, Admission7, CoverageAbi, Fact7, Inventory7, Mode7, Requirements7,
            Union7,
        },
        prepared_capabilities::{self as capabilities, RegistryContract, Requirements},
    },
    pcnd::layer_appearance::{self as coverage7, AdoptedCoverage7, CandidateProvenance},
};
use paintcraft_render::noise_work::WorkComponents;
use protocol::appearance_programs as wire;
mod admission;
mod begin;
mod control;
mod copy;
mod execution;
mod frame;
mod open;
mod owner;
mod policy;
mod quote;
use admission::{Mode, OpenJob};
use owner::Shared;
use policy::SlicePolicy;
#[derive(Debug)]
struct State {
    source: Source,
    base: Document,
    candidate: Option<paintcraft_editor::execute::metadata_clone::StagedMetadata>,
    frame: Option<frame::AppearanceFrame>,
    last_binding: Option<Value>,
    last_copy_admission: Option<Value>,
    generation: Option<u64>,
    profile: wire::Profile,
    adopted: AdoptedCoverage7,
    admission_sha256: [u8; 32],
    job: OpenJob,
    execution: execution::ExecutionAuthority,
    _workspace: BudgetScope,
    output: BudgetScope,
    inventory: BudgetScope,
    metadata_stage: BudgetScope,
}
#[derive(Debug)]
pub struct LayerAppearanceService {
    state: Box<State>,
    baseline: Shared<BudgetAccount>,
    // All State/counter/buffers freed before the final shared controls callback.
    controls: Shared<BudgetAccount>,
}
#[derive(Clone, Copy, Debug)]
pub struct ControlQuote {
    pub retained_bytes: u64,
    pub handle_bytes: u64,
    pub frame_bytes: u64,
    pub extra_output_bytes: u64,
}
#[derive(Debug)]
pub struct CompletedFrame {
    binding: Value,
    pixels: Vec<u8>,
    _output: BudgetAccount,
    _job: OpenJob,
    _baseline: Shared<BudgetAccount>,
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
impl LayerAppearanceService {
    pub fn control_quote() -> Result<ControlQuote> {
        let retained_bytes = (std::mem::size_of::<State>() as u64)
            .checked_add(std::mem::size_of::<Self>() as u64)
            .and_then(|n| n.checked_add(std::mem::size_of::<CompletedFrame>() as u64))
            .ok_or_else(|| resource("Source7 handle quote"))?
            .checked_add(owner::arc_bytes::<BudgetAccount>()?)
            .and_then(|n| n.checked_add(owner::arc_bytes::<BudgetAccount>().ok()?))
            // New7 honestly prices the one base counter allocation on root mint.
            // Inherited owner needs no new allocation; same bound is conservative.
            .and_then(|n| {
                n.checked_add(
                    owner::arc_bytes::<std::sync::Mutex<u64>>()
                        .ok()?
                        .checked_mul(2)?,
                )
            })
            .ok_or_else(|| resource("Source7 retained controls"))?;
        Ok(ControlQuote {
            retained_bytes,
            handle_bytes: std::mem::size_of::<Self>() as u64,
            frame_bytes: std::mem::size_of::<frame::AppearanceFrame>() as u64,
            extra_output_bytes: OUTPUT_PIXELS_MAX * 4,
        })
    }
    pub fn fixed_control_bytes() -> Result<u64> {
        wire::CONTROL_BYTES
            .checked_add(Self::control_quote()?.retained_bytes)
            .ok_or_else(|| resource("Source7 fixed controls"))
    }
    pub fn runtime_facts(noise: bool) -> Result<Value> {
        Ok(Value::object([
            ("protocol", Value::Int(7)),
            ("registryContract", Value::Int(3)),
            ("appearanceRevision", Value::Int(1)),
            ("queryRevision", Value::Int(1)),
            ("coverageAbi", Value::str(CoverageAbi::current()?.id())),
            ("sliceRevision", Value::Int(1)),
            ("nativeEdge", Value::Int(64)),
            (
                "baseSliceWork",
                Value::str(Admission7::BASE_SLICE.to_string()),
            ),
            (
                "noiseSliceWork",
                Value::str(if noise { Admission7::NOISE_SLICE } else { 0 }.to_string()),
            ),
            (
                "controlQuoteBytes",
                Value::str(Self::fixed_control_bytes()?.to_string()),
            ),
        ]))
    }
    pub fn ready(noise: bool) -> Result<Value> {
        Ok(Value::object([
            ("runtimeFacts", Self::runtime_facts(noise)?),
            (
                "controlQuote",
                Value::str(Self::fixed_control_bytes()?.to_string()),
            ),
        ]))
    }
    pub fn capabilities(control: &str, check: &dyn Fn() -> Result<()>) -> Result<Value> {
        wire::hello(control, Self::fixed_control_bytes()?, check)
    }
    pub fn open(bytes: &[u8], control: &str) -> Result<Self> {
        Self::open_checked(bytes, control, &|| Ok(()))
    }
    pub fn open_checked(
        bytes: &[u8],
        control: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        open::open(bytes, control, check)
    }
    pub fn context(&self) -> &Value {
        self.state
            .last_binding
            .as_ref()
            .unwrap_or(&self.state.source.value)
    }
    pub fn prepare(&mut self, control: &str) -> Result<&Value> {
        self.prepare_checked(control, &|| Ok(()))
    }
    pub fn prepare_checked(
        &mut self,
        control: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<&Value> {
        begin::prepare(self, control, check)?;
        self.state
            .last_binding
            .as_ref()
            .ok_or_else(|| bad("Source7 binding missing"))
    }
    pub fn step(&mut self) -> Result<bool> {
        self.step_checked(&|| Ok(()))
    }
    pub fn step_checked(&mut self, check: &dyn Fn() -> Result<()>) -> Result<bool> {
        let doc = self
            .state
            .candidate
            .as_ref()
            .map(|c| c.document())
            .unwrap_or(&self.state.base);
        let result = self
            .state
            .frame
            .as_mut()
            .ok_or_else(|| bad("No Source7 frame"))?
            .step(doc, check);
        if result.is_err() {
            self.retire();
        }
        result
    }
    fn ensure_no_escape(&self) -> Result<()> {
        if self.baseline.strong_count() != 1 || self.controls.strong_count() != 1 {
            return Err(resource(
                "Release Source7 completed result before another preparation",
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
            return Err(bad("Source7 frame incomplete"));
        }
        let f = self.state.frame.take().expect("checked frame");
        if f.copy.is_none() && f.encoded_account.is_none() {
            self.state.execution.success();
        }
        Ok(CompletedFrame {
            binding: f.binding,
            pixels: f.pixels,
            _output: f.output,
            _job: f.job,
            _baseline: self.baseline.clone(),
            _controls: self.controls.clone(),
        })
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn copy_frame_for_wasm(&mut self) -> Result<Vec<u8>> {
        self.ensure_no_escape()?;
        let f = self
            .state
            .frame
            .as_mut()
            .ok_or_else(|| bad("No Source7 frame"))?;
        if !f.complete() || f.delivered {
            return Err(bad("Source7 frame not readable"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(f.pixels.len())
            .map_err(|_| resource("Source7 transfer"))?;
        if bytes.capacity() as u64 > OUTPUT_PIXELS_MAX * 4 {
            return Err(resource("Source7 transfer capacity"));
        }
        bytes.extend_from_slice(&f.pixels);
        f.delivered = true;
        if f.copy.is_none() && f.encoded_account.is_none() {
            self.state.execution.success();
        }
        Ok(bytes)
    }
    pub fn cancel(&mut self, generation: u64) -> Result<()> {
        if self.state.generation != Some(generation) {
            return Err(stale());
        }
        self.state.execution.fail();
        self.retire();
        Ok(())
    }
    pub fn retire(&mut self) {
        self.state.execution.retire();
        self.state.frame = None;
        self.state.candidate = None;
        self.state.last_binding = None;
        self.state.last_copy_admission = None;
        // Storage and captured authority survive; default eligibility is recorded, never reset here.
    }
}
#[cfg(test)]
#[path = "layer_service/tests.rs"]
mod tests;
