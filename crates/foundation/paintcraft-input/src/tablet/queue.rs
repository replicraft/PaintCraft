use super::*;
use std::collections::VecDeque;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    Accepted,
    Duplicate,
    PreviewOnly,
    Busy,
    Hover,
}
/// One captured pointer. Storage is admitted once, including one cancellation
/// slot. A failed gesture stays fenced until Up/Cancel or cancel_capture().
#[derive(Debug)]
pub struct Queue {
    samples: VecDeque<Sample>,
    capacity: usize,
    active: Option<Identity>,
    last: Option<Sample>,
    poisoned: bool,
    frame: Option<u64>,
}
impl Queue {
    pub fn new(capacity: usize) -> Result<Self, Invalid> {
        if !(2..=65_537).contains(&capacity) {
            return Err(Invalid::Capacity);
        }
        let mut samples = VecDeque::new();
        samples
            .try_reserve_exact(capacity)
            .map_err(|_| Invalid::Capacity)?;
        Ok(Self {
            samples,
            capacity,
            active: None,
            last: None,
            poisoned: false,
            frame: None,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.samples.capacity() * std::mem::size_of::<Sample>()
    }
    pub fn active(&self) -> Option<Identity> {
        self.active
    }
    pub fn pen_active(&self) -> bool {
        self.active.is_some_and(|id| id.kind.is_pen())
    }
    pub fn push(&mut self, sample: Sample) -> Result<Admission, Invalid> {
        if sample.provenance == Provenance::Predicted {
            sample.validate()?;
            return Ok(Admission::PreviewOnly);
        }
        if let Some(active) = self.active
            && sample.identity != active
        {
            return Ok(Admission::Busy);
        }
        // A fenced physical tail may itself have malformed attributes. Its
        // terminal phase releases capture but never becomes committed geometry.
        if self.poisoned {
            if matches!(sample.phase, Phase::Up | Phase::Cancel) {
                self.clear_capture();
            }
            return Ok(Admission::Busy);
        }
        if let Err(error) = sample.validate() {
            self.cancel_from_last();
            return Err(error);
        }
        if sample.sequence == u64::MAX || sample.provenance == Provenance::Boundary {
            self.cancel_from_last();
            return Err(Invalid::Order);
        }
        if self.active.is_none() {
            if matches!(sample.phase, Phase::Hover | Phase::Enter | Phase::Leave) {
                return Ok(Admission::Hover);
            }
            if sample.phase != Phase::Down {
                return Ok(Admission::Busy);
            }
        }
        if let Some(last) = self.last.filter(|last| self.active == Some(last.identity)) {
            if last.time.clock != sample.time.clock || last.time.epoch != sample.time.epoch {
                self.cancel_from_last();
                return Err(Invalid::Clock);
            }
            let mut comparable = last;
            comparable.sequence = sample.sequence;
            comparable.provenance = sample.provenance;
            if sample.sequence >= last.sequence && comparable == sample {
                self.last = Some(sample);
                return Ok(Admission::Duplicate);
            }
            if sample.time.micros < last.time.micros || sample.sequence <= last.sequence {
                self.cancel_from_last();
                return Err(Invalid::Order);
            }
            if sample.phase == Phase::Down {
                self.cancel_from_last();
                return Err(Invalid::Lifecycle);
            }
        }
        if self.samples.len() >= self.capacity - 1 {
            self.cancel_from_last();
            return Err(Invalid::Capacity);
        }
        if sample.phase == Phase::Down {
            self.active = Some(sample.identity);
        }
        // The constructor reserved the whole bounded storage; this cannot grow.
        self.samples.push_back(sample);
        self.last = Some(sample);
        if matches!(sample.phase, Phase::Up | Phase::Cancel) {
            self.clear_capture();
        }
        Ok(Admission::Accepted)
    }
    fn clear_capture(&mut self) {
        self.active = None;
        self.last = None;
        self.poisoned = false;
    }
    fn cancel_from_last(&mut self) {
        if self.poisoned {
            return;
        }
        if let Some(mut boundary) = self.last.filter(|last| self.active == Some(last.identity)) {
            boundary.phase = Phase::Cancel;
            boundary.provenance = Provenance::Boundary;
            // A control boundary at the last known valid source time, not an
            // invented hardware timestamp from the malformed incoming event.
            boundary.sequence += 1;
            boundary.pressure = None;
            boundary.tilt = None;
            boundary.twist_degrees = None;
            boundary.contact = None;
            boundary.buttons = Buttons(0);
            debug_assert!(boundary.validate().is_ok());
            debug_assert!(self.samples.len() < self.capacity);
            self.samples.push_back(boundary);
            self.last = Some(boundary);
            self.poisoned = true;
        }
    }
    /// Focus loss, capture loss or disconnect explicitly closes ownership.
    pub fn cancel_capture(&mut self) {
        self.cancel_from_last();
        self.clear_capture();
    }
    /// Fallible frame handoff with optional caller payload admission. Repeated
    /// egui passes see no duplicates. Allocation failure preserves the queue and
    /// appends a valid cancellation rather than allowing partial publication.
    pub fn take_admitted(
        &mut self,
        frame: u64,
        admit: impl FnOnce(usize) -> Result<(), Invalid>,
    ) -> Result<Vec<Sample>, Invalid> {
        if self.frame == Some(frame) {
            return Ok(Vec::new());
        }
        let count = self.samples.len();
        if let Err(error) = admit(count * std::mem::size_of::<Sample>()) {
            self.cancel_from_last();
            return Err(error);
        }
        let mut output = Vec::new();
        if output.try_reserve_exact(count).is_err() {
            self.cancel_from_last();
            return Err(Invalid::Capacity);
        }
        output.extend(self.samples.drain(..));
        self.frame = Some(frame);
        Ok(output)
    }
    pub fn take(&mut self, frame: u64) -> Result<Vec<Sample>, Invalid> {
        self.take_admitted(frame, |_| Ok(()))
    }
}
