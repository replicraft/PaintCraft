//! One validated native message is one indivisible delivery handoff.
use crate::{CaptureEvent, Emitter, delivery::Failure};

pub(super) fn deliver(
    emitter: &Emitter,
    events: &[CaptureEvent],
    owned: bool,
    still_current: impl FnOnce() -> bool,
) -> bool {
    let emission = emitter.gate.emit_checked(|| {
        for event in events {
            if let CaptureEvent::Tablet(packet) = event {
                emitter.report.sample(packet.raw);
            }
            emitter.record(*event)?;
        }
        // A native callback that reentered or lost its installation cannot
        // expose even a complete Up prefix to a concurrent publication handoff.
        if !still_current() {
            return Err(Failure::Source);
        }
        Ok(owned)
    });
    emitter.finish(emission)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Backend, Boundary, DeliveryHealth, RecordFault, Report};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    fn event() -> CaptureEvent {
        CaptureEvent::Boundary {
            kind: Boundary::Keyboard,
            time: None,
        }
    }
    #[test]
    fn partial_sink_refusal_is_failed_before_a_handoff_can_observe_the_prefix() {
        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let a = entered.clone();
        let b = release.clone();
        let c = count.clone();
        let emitter = Emitter::new_records(
            Arc::new(move |_| {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    a.wait();
                    b.wait();
                    Ok(())
                } else {
                    Err(RecordFault::Capacity)
                }
            }),
            Arc::new(|| {}),
            Report::new(Backend::WindowsPointer),
        );
        let producer = emitter.clone();
        let join =
            std::thread::spawn(move || deliver(&producer, &[event(), event()], true, || true));
        entered.wait();
        let gate = emitter.gate.clone();
        let handoff = std::thread::spawn(move || gate.with_delivery(|health| health));
        release.wait();
        assert!(join.join().unwrap());
        assert_eq!(handoff.join().unwrap(), DeliveryHealth::Failed);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn end_of_callback_currentness_fences_before_unlock_and_blocks_future_records() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let copy = seen.clone();
        let emitter = Emitter::new_records(
            Arc::new(move |r| {
                copy.lock().unwrap().push(r.ordinal);
                Ok(())
            }),
            Arc::new(|| {}),
            Report::new(Backend::WindowsPointer),
        );
        assert!(deliver(&emitter, &[event(), event()], false, || false));
        assert_eq!(emitter.gate.with_delivery(|h| h), DeliveryHealth::Failed);
        deliver(&emitter, &[event()], true, || true);
        assert_eq!(*seen.lock().unwrap(), [0, 1]);
        assert_eq!(emitter.report.snapshot().source_failures, 1);
    }
    #[test]
    fn sink_panic_cannot_escape_or_publish_a_valid_prefix() {
        let woke = Arc::new(AtomicBool::new(false));
        let signal = woke.clone();
        let emitter = Emitter::new_records(
            Arc::new(|_| panic!("intentional ingress refusal")),
            Arc::new(move || {
                signal.store(true, Ordering::SeqCst);
            }),
            Report::new(Backend::WindowsPointer),
        );
        assert!(deliver(&emitter, &[event()], true, || true));
        assert!(woke.load(Ordering::SeqCst));
        assert_eq!(emitter.gate.with_delivery(|h| h), DeliveryHealth::Failed);
        assert_eq!(emitter.report.snapshot().sink_failures, 1);
    }
}
