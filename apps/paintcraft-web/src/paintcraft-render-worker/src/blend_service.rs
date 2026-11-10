//! Compiled Source8-only staged display endpoint. No legacy route is reused.
use crate::source8::{
    Source8RgbaImage,
    service_endpoint::{Expected8, LoadedSource8},
};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetAccount, BudgetScope, SharedBudget},
};
use paintcraft_render::noise_work::WorkComponents;
use sha2::{Digest, Sha256};
use std::{cell::Cell, marker::PhantomData, rc::Rc};
mod controls;
mod job;
mod manifest;
mod operation;
mod patch;
mod policy;
mod roles;
mod stage;
use controls::CompiledControls8;
use job::OperationJob8;
use manifest::{Command8, Open8, W};
use operation::{ActiveView8, PreparedView8};
pub use patch::{control_fact, profile_word, runtime_fact};
use policy::*;
use roles::Roles8;
use stage::Stage8;
thread_local! {static LIVE:Cell<bool>=const {Cell::new(false)};}
struct Singleton8(PhantomData<Rc<()>>);
impl Singleton8 {
    fn acquire() -> Result<Self> {
        LIVE.with(|live| {
            if live.replace(true) {
                Err(bound())
            } else {
                Ok(Self(PhantomData))
            }
        })
    }
}
impl Drop for Singleton8 {
    fn drop(&mut self) {
        LIVE.with(|live| live.set(false));
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Phase8 {
    Empty,
    Filling,
    Opened,
    Viewing,
    AwaitAck,
    Complete,
    Cancelled,
    FailedOpen,
    FailedView,
    Closed,
}
/// Private accepted-dispatch capability. A missing ambient context alone cannot create it.
struct StandaloneDispatch8 {
    kind: u8,
}
/// Persistent Source8 source plus one immutable-job view. No source domain escapes.
#[repr(C)]
pub struct LayerBlendService {
    window: [u8; WINDOW],
    controls: CompiledControls8,
    profile: [u8; 32],
    open: Option<Open8>,
    image: Option<Source8RgbaImage>,
    stage: Option<Stage8>,
    source: Option<LoadedSource8>,
    view: Option<ActiveView8>,
    dispatch_job: Option<OperationJob8>,
    last_visible: [u32; 4],
    last_level: u32,
    stats: WorkComponents,
    phase: Phase8,
    last_request: u32,
    last_token: u64,
    last_generation: u64,
    // Payload/accounts end before every original factory, then singleton last.
    roles: Option<Roles8>,
    _control: BudgetAccount,
    h: BudgetScope,
    budget: SharedBudget,
    _singleton: Singleton8,
}
impl std::fmt::Debug for LayerBlendService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerBlendService")
            .field("phase", &self.phase)
            .field("request", &self.last_request)
            .field("token", &self.last_token)
            .finish()
    }
}
impl LayerBlendService {
    /// Fixed compiled discovery. This creates no source, budget, factory or job.
    pub fn control_quote() -> Result<[u64; 3]> {
        let c = CompiledControls8::query()?;
        Ok([c.service_bytes, c.stage_bytes, c.view_bytes])
    }
    /// Actual compiled profile bytes, not wire-provided policy authority.
    pub fn compiled_profile() -> Result<[u8; 32]> {
        Ok(CompiledControls8::query()?.profile())
    }
    /// One instance per module/thread realm; initial controls precede intake.
    pub fn new() -> Result<Self> {
        let singleton = Singleton8::acquire()?;
        let controls = CompiledControls8::query()?;
        let total = controls.total()?;
        let budget = SharedBudget::new(PAYLOAD);
        let h = roles::reserve(&budget, total)?;
        let mut control = h.account();
        control.resize(controls.service_bytes)?;
        Ok(Self {
            window: [0; WINDOW],
            profile: controls.profile(),
            controls,
            open: None,
            image: None,
            stage: None,
            source: None,
            view: None,
            dispatch_job: None,
            last_visible: [0; 4],
            last_level: 0,
            stats: WorkComponents::default(),
            phase: Phase8::Empty,
            last_request: 0,
            last_token: 0,
            last_generation: 0,
            roles: None,
            _control: control,
            h,
            budget,
            _singleton: singleton,
        })
    }
    fn ambient_none() -> Result<()> {
        if paintcraft_render::spatial::capture_current_workspace().is_some()
            || paintcraft_render::noise_work::capture_current_noise_workspace().is_some()
        {
            Err(missing())
        } else {
            Ok(())
        }
    }
    fn command(&self, kind: u8) -> Result<Command8> {
        let c = Command8::decode(&self.window)?;
        if c.kind != kind || c.request <= self.last_request || (kind != 5 && c.request == u32::MAX)
        {
            return Err(stale());
        }
        Ok(c)
    }
    fn snapshot(&mut self) {
        let used = self
            .view
            .as_ref()
            .map(|v| v.job.used())
            .or_else(|| self.dispatch_job.as_ref().map(OperationJob8::used));
        if let Some(Ok(used)) = used {
            self.stats = used;
        }
    }
    fn fail_open(&mut self) {
        self.snapshot();
        self.image = None;
        self.stage = None;
        self.source = None;
        self.dispatch_job = None;
        self.roles = None;
        self.phase = Phase8::FailedOpen;
    }
    fn fail_view(&mut self) {
        self.snapshot();
        self.image = None;
        self.view = None;
        self.dispatch_job = None;
        self.phase = Phase8::FailedView;
    }
    /// Borrow the compiled fixed control window. The trusted host drops views before reentry.
    pub fn has_open_manifest(&self) -> bool {
        self.open.is_some()
    }
    /// Fixed module/thread singleton observation only, never generic joined-credit proof.
    pub fn live_instances() -> u32 {
        LIVE.with(|live| u32::from(live.get()))
    }
    /// The actual fixed metadata memory window.
    pub fn control_ptr(&mut self) -> usize {
        self.window.as_mut_ptr() as usize
    }
    /// Exactly512 control metadata bytes; never a source allocation request.
    pub const fn control_len(&self) -> usize {
        WINDOW
    }
    /// Admit the original whole job before allocating any encoded bytes.
    pub fn stage(&mut self) -> Result<u32> {
        Self::ambient_none()?;
        if self.phase != Phase8::Empty {
            return Err(bad());
        }
        let command = self.command(1)?;
        command.open(self.controls, self.profile)?;
        let dispatch = StandaloneDispatch8 { kind: 1 };
        self.last_request = command.request;
        self.open = Some(command.open);
        self.phase = Phase8::FailedOpen;
        let result = (|| {
            let w = roles::reserve(&self.budget, command.open.roles[W])?;
            let job = OperationJob8::new(&dispatch, &command.open, &w, &self.h)?;
            self.dispatch_job = Some(job);
            let job = self.dispatch_job.as_ref().ok_or_else(bad)?;
            job.spend(FIXED)?;
            job.spend(self.controls.stage_formation_work()?)?;
            self.roles = Some(Roles8::new(&self.budget, &command.open, w)?);
            let roles = self.roles.as_ref().ok_or_else(bad)?;
            self.stage = Some(Stage8::new(
                command.open.bytes,
                &roles.encoded,
                &self.h,
                self.controls.stage_named()?,
                job,
                &|| Ok(()),
            )?);
            Ok(())
        })();
        if let Err(e) = result {
            self.fail_open();
            return Err(e);
        }
        self.snapshot();
        self.phase = Phase8::Filling;
        Ok(0)
    }
    /// Available only during Filling; no Rust call may resize/reenter while borrowed.
    pub fn source_ptr(&mut self) -> Result<usize> {
        if self.phase != Phase8::Filling {
            return Err(bad());
        }
        Ok(self.stage.as_mut().ok_or_else(bad)?.data.as_mut_ptr() as usize)
    }
    /// The exact admitted source extent, without handing a Vec to glue.
    pub fn source_len(&self) -> Result<usize> {
        if self.phase != Phase8::Filling {
            return Err(bad());
        }
        Ok(self.stage.as_ref().ok_or_else(bad)?.data.len())
    }
    /// Authenticate staged bytes and perform one real read/adoption under the OPEN job.
    pub fn seal(&mut self) -> Result<u32> {
        self.seal_checked(&|| Ok(()))
    }
    fn seal_checked(&mut self, check: &dyn Fn() -> Result<()>) -> Result<u32> {
        Self::ambient_none()?;
        if self.phase != Phase8::Filling {
            return Err(bad());
        }
        let result = (|| {
            let open = self.open.as_ref().ok_or_else(bad)?;
            let job = self.dispatch_job.as_ref().ok_or_else(bad)?;
            job.spend(FIXED)?;
            job.spend(
                self.controls
                    .stage_named()?
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(FIXED + RETIRE))
                    .ok_or_else(bound)?,
            )?;
            let stage = self.stage.as_ref().ok_or_else(bad)?;
            if stage.data.len() < 8 || &stage.data[..8] != b"PCND\x08\r\n\x1a" {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Source8 encoded format",
                ));
            }
            let mut hash = Sha256::new();
            for chunk in stage.data.chunks(4096) {
                job.spend(
                    (chunk.len() as u64)
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(512))
                        .ok_or_else(bound)?,
                )?;
                check()?;
                hash.update(chunk);
            }
            if <[u8; 32]>::from(hash.finalize()) != open.binding.source_sha {
                return Err(stale());
            }
            let roles = self.roles.as_ref().ok_or_else(bad)?;
            let expected = Expected8 {
                source_sha: open.binding.source_sha,
                document: open.binding.document,
                state: open.binding.source_state,
                canvas: open.canvas,
                stored_noise: open.stored_noise(),
            };
            let source = job.run(|| {
                LoadedSource8::open_checked(
                    &stage.data,
                    job.request(),
                    roles.read(open),
                    &expected,
                    check,
                    &|n| job.spend(n),
                )
            })?;
            self.source = Some(source);
            Ok(())
        })();
        if let Err(e) = result {
            self.fail_open();
            return Err(e);
        }
        self.snapshot();
        self.stage = None;
        self.dispatch_job = None;
        self.phase = Phase8::Opened;
        Ok(1)
    }
    /// A strictly-new accepted VIEW is the only new render operation/counter.
    pub fn begin_view(&mut self) -> Result<u32> {
        Self::ambient_none()?;
        if !matches!(
            self.phase,
            Phase8::Opened | Phase8::Complete | Phase8::Cancelled
        ) {
            return Err(bad());
        }
        let command = self.command(2)?;
        command.original(self.open.as_ref().ok_or_else(bad)?)?;
        if command.token <= self.last_token || command.generation <= self.last_generation {
            return Err(stale());
        }
        // Fixed command comparisons only before the original dispatch is accepted.
        if command.ordinal != 0
            || command.level > 20
            || command.visible[0] >= command.visible[2]
            || command.visible[1] >= command.visible[3]
            || command.visible[2] > command.open.canvas[0]
            || command.visible[3] > command.open.canvas[1]
        {
            return Err(bad());
        }
        let dispatch = StandaloneDispatch8 { kind: 2 };
        self.image = None;
        self.view = None;
        self.last_request = command.request;
        self.last_token = command.token;
        self.last_generation = command.generation;
        self.last_visible = command.visible;
        self.last_level = command.level;
        self.phase = Phase8::FailedView;
        let result = (|| {
            let roles = self.roles.as_ref().ok_or_else(bad)?;
            self.dispatch_job = Some(OperationJob8::new(
                &dispatch,
                &command.open,
                &roles.w,
                &self.h,
            )?);
            let prepared = PreparedView8::new(
                self.source.as_ref().ok_or_else(bad)?,
                roles,
                &self.h,
                &command,
                self.controls,
                self.dispatch_job.as_ref().ok_or_else(bad)?,
                &|| Ok(()),
            )?;
            let job = self.dispatch_job.take().ok_or_else(bad)?;
            self.view = Some(prepared.finish(job));
            Ok(())
        })();
        if let Err(e) = result {
            self.fail_view();
            return Err(e);
        }
        self.snapshot();
        self.phase = Phase8::Viewing;
        Ok(2)
    }
    /// Render one server-generated patch; one outstanding image prevents further work.
    pub fn step(&mut self) -> Result<u32> {
        self.step_checked(&|| Ok(()))
    }
    fn step_checked(&mut self, check: &dyn Fn() -> Result<()>) -> Result<u32> {
        Self::ambient_none()?;
        if self.phase != Phase8::Viewing {
            return Err(bad());
        }
        let result = (|| {
            let open = self.open.as_ref().ok_or_else(bad)?;
            let active = self.view.as_mut().ok_or_else(bad)?;
            active.job.spend(FIXED)?;
            active.job.spend(self.controls.patch_work()?)?;
            let patch = active.grid.patch(active.ordinal)?;
            let frame = self.roles.as_ref().ok_or_else(bad)?.frame(open);
            let source = self.source.as_ref().ok_or_else(bad)?;
            let image = active
                .job
                .run(|| active.view.render_checked(source, &frame, &patch, check))?;
            self.image = Some(image);
            Ok(())
        })();
        if let Err(e) = result {
            self.fail_view();
            return Err(e);
        }
        self.snapshot();
        self.phase = Phase8::AwaitAck;
        Ok(3)
    }
    /// Only the currently outstanding immutable image can be borrowed.
    pub fn rgba_ptr(&self) -> Result<usize> {
        if self.phase != Phase8::AwaitAck {
            return Err(bad());
        }
        Ok(self.image.as_ref().ok_or_else(bad)?.rgba().as_ptr() as usize)
    }
    /// Native/display patch length is bounded to64x64 straightRGBA cells.
    pub fn rgba_len(&self) -> Result<usize> {
        if self.phase != Phase8::AwaitAck {
            return Err(bad());
        }
        Ok(self.image.as_ref().ok_or_else(bad)?.rgba().len())
    }
    /// Exact current ordinal/rectangle/token/generation/whole-view size, no source escape.
    pub fn patch_fact(&self, index: u32) -> Result<u64> {
        if self.phase != Phase8::AwaitAck {
            return Err(bad());
        }
        let active = self.view.as_ref().ok_or_else(bad)?;
        let patch = active.grid.patch(active.ordinal)?;
        let values = [
            u64::from(active.ordinal),
            patch.patch.x0 as u64,
            patch.patch.y0 as u64,
            patch.patch.x1 as u64,
            patch.patch.y1 as u64,
            active.token,
            active.generation,
            u64::from(active.grid.size[0]),
            u64::from(active.grid.size[1]),
            u64::from(active.grid.patches),
        ];
        values.get(index as usize).copied().ok_or_else(bad)
    }
    /// A validated ACK retires data first; it does not refund/renew any operation.
    pub fn ack(&mut self) -> Result<u32> {
        if self.phase != Phase8::AwaitAck {
            return Err(bad());
        }
        let command = self.command(3)?;
        let active = self.view.as_mut().ok_or_else(bad)?;
        active.matches(&command, self.open.as_ref().ok_or_else(bad)?)?;
        if command.ordinal != active.ordinal {
            return Err(stale());
        }
        self.image = None;
        active.ordinal += 1;
        self.last_request = command.request;
        self.phase = if active.ordinal == active.grid.patches {
            Phase8::Complete
        } else {
            Phase8::Viewing
        };
        Ok(if self.phase == Phase8::Complete { 4 } else { 2 })
    }
    /// Cancellation joins this synchronous owner and is terminal for its token.
    pub fn cancel(&mut self) -> Result<u32> {
        if !matches!(
            self.phase,
            Phase8::Viewing | Phase8::AwaitAck | Phase8::FailedView
        ) {
            return Err(bad());
        }
        let command = self.command(4)?;
        command.original(self.open.as_ref().ok_or_else(bad)?)?;
        if command.token != self.last_token
            || command.generation != self.last_generation
            || command.ordinal != 0
            || command.visible != self.last_visible
            || command.level != self.last_level
        {
            return Err(stale());
        }
        self.last_request = command.request;
        self.snapshot();
        self.image = None;
        self.view = None;
        self.phase = Phase8::Cancelled;
        Ok(5)
    }
    /// Cached diagnostic work snapshot only; never an admission or remaining grant.
    pub fn work_used(&self, noise: bool) -> u64 {
        if noise {
            self.stats.noise
        } else {
            self.stats.base
        }
    }
    /// Consume every data owner and original factory before a caller can acknowledge CLOSED.
    pub fn close(mut self) -> Result<u32> {
        let valid = (|| {
            let command = self.command(5)?;
            if let Some(open) = &self.open {
                command.original(open)?;
            } else if self.window[7] != 0 || self.window[16..344].iter().any(|b| *b != 0) {
                return Err(bad());
            }
            if command.token != 0
                || command.generation != 0
                || command.visible != [0; 4]
                || command.level != 0
                || command.ordinal != 0
            {
                return Err(bad());
            }
            Ok(())
        })();
        self.clear();
        valid.map(|_| 6)
    }
    fn clear(&mut self) {
        self.snapshot();
        self.image = None;
        self.stage = None;
        self.view = None;
        self.source = None;
        self.dispatch_job = None;
        self.roles = None;
        self.phase = Phase8::Closed;
    }
}
impl Drop for LayerBlendService {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
#[path = "blend_service/tests.rs"]
mod tests;
