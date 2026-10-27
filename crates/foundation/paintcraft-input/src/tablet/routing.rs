//! Shared capture arbitration. Platform samples never bypass modal/IME ownership.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct Permissions {
    pub owner: u128,
    pub focused: bool,
    pub canvas_hit: bool,
    pub modal: bool,
    pub ime: bool,
}
#[derive(Debug)]
pub struct Router {
    queue: Queue,
    fingers: [Option<(u64, bool)>; 32],
}
impl Router {
    pub fn new(capacity: usize) -> Result<Self, Invalid> {
        Ok(Self {
            queue: Queue::new(capacity)?,
            fingers: [None; 32],
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.queue.storage_bytes()
    }
    pub fn pending_samples(&self) -> usize {
        self.queue.pending_samples()
    }
    pub(super) fn discard_pending(&mut self) {
        self.queue.discard_pending();
    }
    pub fn pen_active(&self) -> bool {
        self.queue.pen_active()
    }
    pub fn submit(
        &mut self,
        sample: Sample,
        permissions: Permissions,
    ) -> Result<Admission, Invalid> {
        if !permissions.focused || permissions.modal || permissions.ime {
            self.queue.cancel_capture();
            return Ok(Admission::Busy);
        }
        if sample.transform.owner != permissions.owner {
            self.queue.cancel_capture();
            return Err(Invalid::Owner);
        }
        if sample.phase == Phase::Down && !permissions.canvas_hit {
            return Ok(Admission::Busy);
        }
        let admitted = self.queue.push(sample)?;
        if admitted == Admission::Accepted
            && sample.phase == Phase::Down
            && sample.identity.kind.is_pen()
        {
            self.fence_fingers();
        }
        Ok(admitted)
    }
    /// Practical contact palm rejection: never infer pen identity from finger
    /// pressure. Once suppressed, a finger stays fenced through its complete
    /// release tail, including after the pen itself is lifted.
    pub fn finger(&mut self, id: u64, phase: Phase) -> Result<bool, Invalid> {
        self.finger_owned(id, phase, false)
    }
    pub(super) fn fence_fingers(&mut self) {
        for finger in self.fingers.iter_mut().flatten() {
            finger.1 = true;
        }
    }
    pub(super) fn finger_owned(
        &mut self,
        id: u64,
        phase: Phase,
        pen_owns: bool,
    ) -> Result<bool, Invalid> {
        let fence =
            pen_owns || self.pen_active() || self.fingers.iter().flatten().any(|entry| entry.1);
        if let Some(index) = self
            .fingers
            .iter()
            .position(|entry| entry.is_some_and(|entry| entry.0 == id))
        {
            let suppressed = fence || self.fingers[index].unwrap().1;
            if matches!(phase, Phase::Up | Phase::Cancel) {
                self.fingers[index] = None;
            } else {
                self.fingers[index] = Some((id, suppressed));
            }
            return Ok(suppressed);
        }
        if phase == Phase::Down {
            let index = self
                .fingers
                .iter()
                .position(Option::is_none)
                .ok_or(Invalid::Capacity)?;
            self.fingers[index] = Some((id, fence));
        }
        Ok(fence)
    }
    pub fn cancel(&mut self) {
        self.queue.cancel_capture();
    }
    /// Window/capture teardown establishes a new lifecycle for fingers too.
    pub fn reset(&mut self) {
        self.cancel();
        self.fingers = [None; 32];
    }
    pub fn take(&mut self, frame: u64) -> Result<Vec<Sample>, Invalid> {
        self.queue.take(frame)
    }
}
