//! Clock reclamation requires the producer's typed real range-end evidence.
use super::*;
impl Collector {
    pub(super) fn retire_identity(
        &mut self,
        identity: Identity,
        time: Time,
        epoch: u64,
    ) -> std::result::Result<(), RecordFault> {
        if time.epoch != epoch
            || self.owner == Some(identity)
            || self.fingers.held(identity)
            || self
                .pens
                .iter()
                .flatten()
                .any(|pen| pen.identity == identity && (pen.held || pen.fenced))
        {
            return Err(RecordFault::Invalid);
        }
        let index = self
            .times
            .iter()
            .position(|entry| entry.is_some_and(|(id, _)| id == identity))
            .ok_or(RecordFault::Invalid)?;
        if !ordered_time(self.times[index].map(|(_, time)| time), time) {
            return Err(RecordFault::Invalid);
        }
        self.times[index] = None;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::tests::{pen, time};
    #[test]
    fn genuine_completed_lifetimes_reclaim_only_their_own_clock_beyond_sixty_four_ids() {
        let mut collector = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
        for life in 0..128 {
            let mut ordinal = life * 4;
            let mut identity = None;
            for (phase, buttons) in [
                (Phase::Down, Buttons::TIP),
                (Phase::Up, 0),
                (Phase::Leave, 0),
            ] {
                let mut record = pen(ordinal, phase, buttons);
                let CaptureEvent::Tablet(packet) = &mut record.event else {
                    unreachable!()
                };
                packet.raw.identity.pointer = life;
                identity = Some(packet.raw.identity);
                collector.accept(record).unwrap();
                ordinal += 1;
            }
            collector
                .accept(CaptureRecord {
                    installation: 7,
                    ordinal,
                    event: CaptureEvent::IdentityRetired {
                        identity: identity.unwrap(),
                        time: time(),
                    },
                })
                .unwrap();
            let frame = collector.take(DeliveryHealth::Ready).unwrap();
            assert_eq!(frame.records.len(), 4);
            assert!(matches!(frame.records[3], Collected::IdentityRetired(_)));
        }
        assert!(collector.times.iter().all(Option::is_none));
        assert!(!collector.failed());
    }
    #[test]
    fn held_or_clock_mismatched_retirement_refuses_without_publishing_a_prefix() {
        for mismatch in [false, true] {
            let mut collector = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
            let record = pen(
                0,
                if mismatch { Phase::Up } else { Phase::Down },
                if mismatch { 0 } else { Buttons::TIP },
            );
            let CaptureEvent::Tablet(packet) = record.event else {
                unreachable!()
            };
            collector.accept(record).unwrap();
            let mut time = time();
            if mismatch {
                time.micros -= 1;
            }
            assert_eq!(
                collector.accept(CaptureRecord {
                    installation: 7,
                    ordinal: 1,
                    event: CaptureEvent::IdentityRetired {
                        identity: packet.raw.identity,
                        time
                    }
                }),
                Err(RecordFault::Invalid)
            );
            assert!(
                collector
                    .take(DeliveryHealth::Ready)
                    .unwrap()
                    .records
                    .is_empty()
            );
        }
    }
    #[test]
    fn generic_idle_leave_does_not_rebase_reused_identity_clock() {
        let mut collector = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
        collector.accept(pen(0, Phase::Down, Buttons::TIP)).unwrap();
        collector.accept(pen(1, Phase::Up, 0)).unwrap();
        collector.accept(pen(2, Phase::Leave, 0)).unwrap();
        let mut next = pen(3, Phase::Down, Buttons::TIP);
        let CaptureEvent::Tablet(packet) = &mut next.event else {
            unreachable!()
        };
        packet.raw.time.micros -= 1;
        assert_eq!(collector.accept(next), Err(RecordFault::Invalid));
    }
}
