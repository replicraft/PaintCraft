//! Time-based view movement during an owned tool drag, independent of the host UI.
use super::{Point, finite};

#[derive(Clone, Debug)]
struct Drag<S> {
    scope: S,
    origin: Point,
    time: f64,
}

#[derive(Clone, Debug)]
pub struct AutoScroll<S> {
    drag: Option<Drag<S>>,
}
impl<S> Default for AutoScroll<S> {
    fn default() -> Self {
        Self { drag: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoScrollStep {
    /// Logical screen displacement of the image; the scroll adapter applies bounds.
    pub delta: Point,
    /// The image moved under a stationary pointer, so the tool endpoint must follow it.
    pub rebase: bool,
    pub at_edge: bool,
}

impl<S: Eq> AutoScroll<S> {
    pub fn cancel(&mut self) {
        self.drag = None;
    }
    /// Only an actual accepted pointer press may arm auto-scroll.
    pub fn begin(&mut self, scope: S, origin: Point, time: f64) {
        self.cancel();
        if finite(origin) && time.is_finite() {
            self.drag = Some(Drag {
                scope,
                origin,
                time,
            });
        }
    }
    pub fn advance(
        &mut self,
        scope: &S,
        origin: Point,
        pointer: Point,
        viewport: (Point, Point),
        time: f64,
    ) -> Option<AutoScrollStep> {
        let drag = self.drag.as_ref()?;
        if &drag.scope != scope
            || ![origin, pointer, viewport.0, viewport.1]
                .into_iter()
                .all(finite)
            || viewport.1.0 <= viewport.0.0
            || viewport.1.1 <= viewport.0.1
            || !time.is_finite()
            || time < drag.time
        {
            self.cancel();
            return None;
        }
        let drag = self.drag.as_mut()?;
        // A stalled application must not jump a large distance on its next frame.
        let elapsed = (time - drag.time).min(0.05) as f32;
        let velocity = (
            speed(pointer.0, viewport.0.0, viewport.1.0),
            speed(pointer.1, viewport.0.1, viewport.1.1),
        );
        let result = AutoScrollStep {
            delta: (velocity.0 * elapsed, velocity.1 * elapsed),
            rebase: drag.origin != origin,
            at_edge: velocity != (0.0, 0.0),
        };
        drag.origin = origin;
        drag.time = time;
        Some(result)
    }
}

fn speed(at: f32, low: f32, high: f32) -> f32 {
    // Provisional Studio policy: accelerate over the outer 24 logical points,
    // capped at 480 points/second per axis. Tiny viewports retain a neutral center.
    let band = 24.0f32.min((high - low) * 0.25);
    if at < low + band {
        480.0 * ((low + band - at) / band).min(1.0)
    } else if at > high - band {
        -480.0 * ((at - high + band) / band).min(1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const VIEW: (Point, Point) = ((0.0, 0.0), (400.0, 300.0));

    #[test]
    fn movement_uses_elapsed_time_and_each_edge_has_the_correct_direction() {
        for (pointer, velocity) in [
            ((0.0, 150.0), (480.0, 0.0)),
            ((400.0, 150.0), (-480.0, 0.0)),
            ((200.0, 0.0), (0.0, 480.0)),
            ((200.0, 300.0), (0.0, -480.0)),
            ((412.0, 312.0), (-480.0, -480.0)),
            ((12.0, 150.0), (240.0, 0.0)),
        ] {
            for frames in [30, 60, 120] {
                let mut gesture = AutoScroll::default();
                gesture.begin(1, (0.0, 0.0), 0.0);
                let mut total = (0.0, 0.0);
                for frame in 1..=frames {
                    let step = gesture
                        .advance(&1, (0.0, 0.0), pointer, VIEW, frame as f64 / frames as f64)
                        .unwrap();
                    total.0 += step.delta.0;
                    total.1 += step.delta.1;
                }
                assert!((total.0 - velocity.0).abs() < 0.01);
                assert!((total.1 - velocity.1).abs() < 0.01);
            }
        }
    }
    #[test]
    fn origin_changes_rebase_once_and_idle_centers_do_not_scroll() {
        let mut gesture = AutoScroll::default();
        gesture.begin(1, (10.0, 20.0), 0.0);
        let still = gesture
            .advance(&1, (10.0, 20.0), (200.0, 150.0), VIEW, 0.01)
            .unwrap();
        assert_eq!(
            still,
            AutoScrollStep {
                delta: (0.0, 0.0),
                at_edge: false,
                rebase: false
            }
        );
        assert!(
            gesture
                .advance(&1, (8.0, 20.0), (200.0, 150.0), VIEW, 0.02)
                .unwrap()
                .rebase
        );
        assert!(
            !gesture
                .advance(&1, (8.0, 20.0), (200.0, 150.0), VIEW, 0.03)
                .unwrap()
                .rebase
        );
        assert_eq!(
            gesture
                .advance(&1, (8.0, 20.0), (400.0, 150.0), VIEW, 100.0)
                .unwrap()
                .delta,
            (-24.0, 0.0)
        );
    }
    #[test]
    fn cancelled_stale_or_invalid_drags_cannot_restart_without_a_press() {
        let mut gesture = AutoScroll::default();
        gesture.begin(1, (0.0, 0.0), 0.0);
        assert!(
            gesture
                .advance(&2, (0.0, 0.0), (400.0, 150.0), VIEW, 0.1)
                .is_none()
        );
        assert!(
            gesture
                .advance(&1, (0.0, 0.0), (400.0, 150.0), VIEW, 0.2)
                .is_none()
        );
        for time in [f64::NAN, -1.0] {
            gesture.begin(1, (0.0, 0.0), 0.0);
            assert!(
                gesture
                    .advance(&1, (0.0, 0.0), (400.0, 150.0), VIEW, time)
                    .is_none()
            );
        }
        gesture.begin(1, (0.0, 0.0), 0.0);
        gesture.cancel();
        assert!(
            gesture
                .advance(&1, (0.0, 0.0), (400.0, 150.0), VIEW, 0.2)
                .is_none()
        );
    }
}
