//! Captured logical-screen displacement; document coordinates change during pan.
use super::{Point, finite};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanButton {
    Primary,
    Middle,
}

#[derive(Clone, Debug)]
struct Drag<S> {
    scope: S,
    button: PanButton,
    last: Point,
}

#[derive(Clone, Debug)]
pub struct PanGesture<S> {
    drag: Option<Drag<S>>,
}
impl<S> Default for PanGesture<S> {
    fn default() -> Self {
        Self { drag: None }
    }
}
impl<S: Eq> PanGesture<S> {
    pub fn is_active(&self) -> bool {
        self.drag.is_some()
    }
    pub fn cancel(&mut self) {
        self.drag = None;
    }
    pub fn validate(&mut self, scope: &S, enabled: bool) {
        if !enabled || self.drag.as_ref().is_some_and(|drag| &drag.scope != scope) {
            self.cancel();
        }
    }
    pub fn begin(&mut self, scope: S, button: PanButton, at: Point) {
        if self.drag.is_none() && finite(at) {
            self.drag = Some(Drag {
                scope,
                button,
                last: at,
            });
        }
    }
    /// Positive displacement moves the image with the pointer. The host maps
    /// this to a negative scroll offset and retains its viewport bounds policy.
    pub fn move_to(&mut self, scope: &S, at: Point) -> Option<Point> {
        self.validate(scope, true);
        if !finite(at) {
            self.cancel();
            return None;
        }
        let drag = self.drag.as_mut()?;
        let delta = (at.0 - drag.last.0, at.1 - drag.last.1);
        drag.last = at;
        (delta != (0.0, 0.0)).then_some(delta)
    }
    pub fn release(&mut self, scope: &S, button: PanButton, at: Point) -> Option<Point> {
        self.validate(scope, true);
        if self.drag.as_ref()?.button != button {
            return None;
        }
        let delta = self.move_to(scope, at);
        self.cancel();
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deltas_accumulate_once_and_only_the_owner_can_release() {
        let mut pan = PanGesture::default();
        pan.begin(1, PanButton::Middle, (10.0, 20.0));
        pan.begin(1, PanButton::Primary, (100.0, 200.0));
        assert_eq!(pan.move_to(&1, (13.5, 16.0)), Some((3.5, -4.0)));
        assert_eq!(pan.move_to(&1, (13.5, 16.0)), None);
        assert_eq!(pan.release(&1, PanButton::Primary, (100.0, 100.0)), None);
        assert!(pan.is_active());
        assert_eq!(
            pan.release(&1, PanButton::Middle, (15.0, 20.0)),
            Some((1.5, 4.0))
        );
        assert!(!pan.is_active());
    }
    #[test]
    fn stale_source_disabled_input_and_nonfinite_positions_cancel() {
        let mut pan = PanGesture::default();
        pan.begin(1, PanButton::Primary, (0.0, 0.0));
        assert_eq!(pan.move_to(&2, (10.0, 10.0)), None);
        assert!(!pan.is_active());
        pan.begin(2, PanButton::Primary, (0.0, 0.0));
        pan.validate(&2, false);
        assert!(!pan.is_active());
        pan.begin(2, PanButton::Primary, (0.0, 0.0));
        assert_eq!(pan.move_to(&2, (f32::NAN, 0.0)), None);
        assert!(!pan.is_active());
    }
}
