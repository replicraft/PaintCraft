//! One bounded framework interval retained behind an actual service barrier.
//! Host-observed FIFO only: this does not authenticate native key/IME ordinals.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount};
pub(super) struct Retained {
    events: Vec<egui::Event>,
    segments: Vec<Segment>,
    next: u64,
    limit: usize,
    metadata: u64,
    storage: BudgetAccount,
}
struct Segment {
    serial: u64,
    count: usize,
}
impl Retained {
    pub(super) fn new(limit: usize, budget: &SharedBudget) -> Result<Self> {
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(
            (limit * (2 * std::mem::size_of::<egui::Event>() + std::mem::size_of::<Segment>()))
                as u64,
        )?;
        let mut events = Vec::new();
        events
            .try_reserve_exact(limit)
            .map_err(|_| resource("Framework interval reserve"))?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(limit)
            .map_err(|_| resource("Framework segment reserve"))?;
        let metadata = (2 * events.capacity() * std::mem::size_of::<egui::Event>()
            + segments.capacity() * std::mem::size_of::<Segment>()) as u64;
        storage.resize(metadata)?;
        Ok(Self {
            events,
            segments,
            next: 0,
            limit,
            metadata,
            storage,
        })
    }
    pub(super) fn retain(&mut self, incoming: &mut Vec<egui::Event>) -> Result<u64> {
        if self
            .events
            .len()
            .checked_add(incoming.len())
            .is_none_or(|len| len > self.limit)
            || incoming.is_empty()
            || self.segments.len() == self.limit
        {
            return Err(resource("Framework interval event capacity"));
        }
        let serial = self
            .next
            .checked_add(1)
            .ok_or_else(|| resource("Framework interval serial"))?;
        let bytes =
            self.events
                .iter()
                .chain(incoming.iter())
                .try_fold(self.metadata, |sum, event| {
                    sum.checked_add(payload_bytes(event)?)
                        .ok_or_else(|| resource("Framework retained bytes"))
                })?;
        // Outside delivery/ingress locks. Resize failure moves no prefix.
        self.storage.resize(bytes)?;
        self.segments.push(Segment {
            serial,
            count: incoming.len(),
        });
        self.events.append(incoming);
        self.next = serial;
        Ok(serial)
    }
    pub(super) fn clear(&mut self) {
        self.events.clear();
        self.segments.clear();
        self.after_render();
    }
    pub(super) fn pending(&self) -> bool {
        !self.segments.is_empty()
    }
    pub(super) fn replay(&mut self, serial: u64, output: &mut Vec<egui::Event>) -> Result<()> {
        if !output.is_empty() {
            return Err(stale("Framework replay requires an empty interval"));
        }
        let count = self
            .segments
            .first()
            .filter(|segment| segment.serial == serial)
            .ok_or_else(|| stale("Framework interval FIFO mismatch"))?
            .count;
        let old_capacity = output.capacity();
        output
            .try_reserve_exact(count)
            .map_err(|_| resource("Framework replay reserve"))?;
        if output.capacity().saturating_sub(old_capacity) > self.limit {
            return Err(resource("Framework replay allocation capacity"));
        }
        output.extend(self.events.drain(..count));
        self.segments.remove(0);
        Ok(())
    }
    pub(super) fn after_render(&mut self) {
        if self.events.is_empty() {
            // The input interval has left our ownership. egui's own retained
            // state/allocator and OS framework storage are not claimed here.
            self.storage
                .resize(self.metadata)
                .expect("shrinking admitted framework interval");
        }
    }
}
fn payload_bytes(event: &egui::Event) -> Result<u64> {
    let bytes = match event {
        egui::Event::Text(text)
        | egui::Event::Paste(text)
        | egui::Event::Ime(egui::ImeEvent::Commit(text))
        | egui::Event::Ime(egui::ImeEvent::Preedit { text, .. })
        | egui::Event::MiddleClickPaste { text, .. } => text.capacity(),
        egui::Event::PasteImage(image) => image
            .pixels
            .capacity()
            .checked_mul(std::mem::size_of::<egui::Color32>())
            .and_then(|bytes| {
                bytes.checked_add(
                    std::mem::size_of::<egui::ColorImage>() + 2 * std::mem::size_of::<usize>(),
                )
            })
            .ok_or_else(|| resource("Framework clipboard image bytes"))?,
        egui::Event::AccessKitActionRequest(request) => match &request.data {
            Some(egui::accesskit::ActionData::Value(text)) => text.len(),
            _ => 0,
        },
        _ => 0,
    };
    u64::try_from(bytes).map_err(|_| resource("Framework payload length"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn append_preserves_text_key_order_and_denial_moves_no_prefix() {
        let budget = SharedBudget::new(1 << 20);
        let mut retained = Retained::new(2, &budget).unwrap();
        let baseline = budget.usage().reserved;
        let mut first = vec![egui::Event::Text(String::with_capacity(128))];
        let first_id = retained.retain(&mut first).unwrap();
        assert!(first.is_empty());
        assert!(budget.usage().reserved >= baseline + 128);
        let mut second = vec![egui::Event::Copy];
        let second_id = retained.retain(&mut second).unwrap();
        let mut overflow = vec![egui::Event::Cut];
        assert!(retained.retain(&mut overflow).is_err());
        assert!(matches!(overflow.as_slice(), [egui::Event::Cut]));
        let mut output = Vec::new();
        assert!(retained.replay(second_id, &mut output).is_err());
        assert!(output.is_empty());
        retained.replay(first_id, &mut output).unwrap();
        assert!(matches!(output.as_slice(), [egui::Event::Text(_)]));
        output = Vec::with_capacity(64);
        retained.replay(second_id, &mut output).unwrap();
        assert!(matches!(output.as_slice(), [egui::Event::Copy]));
        retained.after_render();
        assert_eq!(budget.usage().reserved, baseline);
        drop(retained);
        assert_eq!(budget.usage().reserved, 0);
    }
}
