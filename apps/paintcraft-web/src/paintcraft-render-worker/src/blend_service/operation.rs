use super::{
    controls::CompiledControls8,
    job::OperationJob8,
    manifest::{Command8, E, H, Open8},
    policy::*,
    roles::Roles8,
};
use crate::source8::{
    Source8DisplayPatch,
    service_endpoint::{LoadedSource8, LoadedView8, Originals8},
};
use paintcraft_core::types::{
    Rect, Result,
    resources::{BudgetAccount, BudgetScope},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Grid8 {
    pub visible: Rect,
    pub region: Rect,
    pub size: [u32; 2],
    pub level: u32,
    pub columns: u32,
    pub patches: u32,
}
impl Grid8 {
    pub fn new(command: &Command8) -> Result<Self> {
        let v = command.visible;
        let canvas = command.open.canvas;
        if command.kind != 2
            || command.token == 0
            || command.generation == 0
            || command.ordinal != 0
            || command.level > 20
            || v[0] >= v[2]
            || v[1] >= v[3]
            || v[2] > canvas[0]
            || v[3] > canvas[1]
        {
            return Err(bad());
        }
        let step = 1u64 << command.level;
        let x0 = u64::from(v[0]) / step * step;
        let y0 = u64::from(v[1]) / step * step;
        let x1 = u64::from(v[2])
            .div_ceil(step)
            .checked_mul(step)
            .ok_or_else(bound)?
            .min(u64::from(canvas[0]));
        let y1 = u64::from(v[3])
            .div_ceil(step)
            .checked_mul(step)
            .ok_or_else(bound)?
            .min(u64::from(canvas[1]));
        let width = (x1 - x0).div_ceil(step);
        let height = (y1 - y0).div_ceil(step);
        if width.checked_mul(height).is_none_or(|n| n > CELLS) {
            return Err(bound());
        }
        let columns = width.div_ceil(64);
        let rows = height.div_ceil(64);
        Ok(Self {
            visible: Rect {
                x0: i64::from(v[0]),
                y0: i64::from(v[1]),
                x1: i64::from(v[2]),
                y1: i64::from(v[3]),
            },
            region: Rect {
                x0: x0 as i64,
                y0: y0 as i64,
                x1: x1 as i64,
                y1: y1 as i64,
            },
            size: [width as u32, height as u32],
            level: command.level,
            columns: columns as u32,
            patches: u32::try_from(columns.checked_mul(rows).ok_or_else(bound)?)
                .map_err(|_| bound())?,
        })
    }
    pub fn patch(&self, ordinal: u32) -> Result<Source8DisplayPatch> {
        if ordinal >= self.patches {
            return Err(bad());
        }
        let x = ordinal % self.columns * 64;
        let y = ordinal / self.columns * 64;
        Ok(Source8DisplayPatch {
            visible: self.visible,
            level: self.level as u8,
            patch: Rect {
                x0: i64::from(x),
                y0: i64::from(y),
                x1: i64::from((x + 64).min(self.size[0])),
                y1: i64::from((y + 64).min(self.size[1])),
            },
        })
    }
    pub fn matches(&self, command: &Command8) -> bool {
        command.visible
            == [
                self.visible.x0 as u32,
                self.visible.y0 as u32,
                self.visible.x1 as u32,
                self.visible.y1 as u32,
            ]
            && command.level == self.level
    }
}
#[repr(C)]
pub(super) struct ActiveView8 {
    pub view: LoadedView8,
    pub job: OperationJob8,
    pub grid: Grid8,
    pub token: u64,
    pub generation: u64,
    pub ordinal: u32,
    _controls: BudgetAccount,
    _factory: BudgetScope,
}
#[repr(C)]
pub(super) struct PreparedView8 {
    view: LoadedView8,
    grid: Grid8,
    token: u64,
    generation: u64,
    controls: BudgetAccount,
    factory: BudgetScope,
}
impl PreparedView8 {
    pub fn new(
        source: &LoadedSource8,
        roles: &Roles8,
        h: &BudgetScope,
        command: &Command8,
        controls: CompiledControls8,
        job: &OperationJob8,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        job.spend(FIXED)?;
        job.spend(
            controls
                .view_named()?
                .checked_mul(2)
                .and_then(|n| n.checked_add(FIXED))
                .ok_or_else(bound)?,
        )?;
        let grid = Grid8::new(command)?;
        job.spend(
            u64::from(grid.patches)
                .checked_mul(ACK)
                .and_then(|n| n.checked_add(RETIRE))
                .ok_or_else(bound)?,
        )?;
        let mut account = h.account();
        account.resize(controls.view_named()?.checked_sub(4096).ok_or_else(bound)?)?;
        let originals = Originals8 {
            encoded: &roles.encoded,
            control: h,
            encoded_capacity: command.open.roles[E],
            control_capacity: command.open.roles[H],
        };
        let frame = roles.frame(&command.open);
        let view = job.run(|| {
            source.begin_checked(&job.request(), &originals, &frame, check, &|n| job.spend(n))
        })?;
        Ok(Self {
            view,
            grid,
            token: command.token,
            generation: command.generation,
            controls: account,
            factory: h.clone(),
        })
    }
    pub fn finish(self, job: OperationJob8) -> ActiveView8 {
        ActiveView8 {
            view: self.view,
            job,
            grid: self.grid,
            token: self.token,
            generation: self.generation,
            ordinal: 0,
            _controls: self.controls,
            _factory: self.factory,
        }
    }
}
impl ActiveView8 {
    pub fn matches(&self, c: &Command8, open: &Open8) -> Result<()> {
        c.original(open)?;
        if c.token != self.token || c.generation != self.generation || !self.grid.matches(c) {
            return Err(stale());
        }
        Ok(())
    }
}
