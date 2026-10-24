//! View-only navigation gestures. Hosts supply scope identity and map view requests.
use crate::PointerButton;
mod pan;
pub use pan::{PanButton, PanDirection, PanGesture, keyboard_pan_delta};

pub type Point = (f32, f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoomRequest {
    Step { center: Point, increase: bool },
    Rectangle { from: Point, to: Point },
}

#[derive(Clone, Debug)]
struct Drag<S> {
    scope: S,
    button: PointerButton,
    from: Point,
    to: Point,
}

/// A source/view change invalidates an in-flight gesture instead of applying it
/// to another document or using a different screen-to-document transform.
#[derive(Clone, Debug)]
pub struct ZoomGesture<S> {
    drag: Option<Drag<S>>,
}

impl<S> Default for ZoomGesture<S> {
    fn default() -> Self {
        Self { drag: None }
    }
}

impl<S: Eq> ZoomGesture<S> {
    pub fn cancel(&mut self) {
        self.drag = None;
    }
    pub fn is_active(&self) -> bool {
        self.drag.is_some()
    }

    pub fn validate(&mut self, scope: &S, enabled: bool) {
        if !enabled || self.drag.as_ref().is_some_and(|drag| &drag.scope != scope) {
            self.cancel();
        }
    }

    pub fn begin(&mut self, scope: S, button: PointerButton, at: Point) {
        if self.drag.is_none() && finite(at) {
            self.drag = Some(Drag {
                scope,
                button,
                from: at,
                to: at,
            });
        }
    }

    pub fn update(&mut self, at: Point) {
        if finite(at)
            && let Some(drag) = &mut self.drag
        {
            drag.to = at;
        }
    }

    /// Four logical screen points distinguish a deliberate box from click jitter.
    /// Secondary drags retain zoom-out click behavior.
    pub fn preview(&self, points_per_pixel: f32) -> Option<(Point, Point)> {
        let drag = self.drag.as_ref()?;
        if drag.button != PointerButton::Primary || !moved(drag, points_per_pixel) {
            return None;
        }
        Some((drag.from, drag.to))
    }

    pub fn release(
        &mut self,
        scope: &S,
        button: PointerButton,
        at: Point,
        points_per_pixel: f32,
    ) -> Option<ZoomRequest> {
        self.validate(
            scope,
            points_per_pixel.is_finite() && points_per_pixel > 0.0,
        );
        if self.drag.as_ref()?.button != button {
            return None;
        }
        if !finite(at) {
            self.cancel();
            return None;
        }
        self.update(at);
        let drag = self.drag.take()?;
        if drag.button == PointerButton::Primary && moved(&drag, points_per_pixel) {
            Some(ZoomRequest::Rectangle {
                from: drag.from,
                to: drag.to,
            })
        } else {
            Some(ZoomRequest::Step {
                center: drag.from,
                increase: button == PointerButton::Primary,
            })
        }
    }
}

fn finite(point: Point) -> bool {
    point.0.is_finite()
        && point.1.is_finite()
        && point.0.abs() <= paintcraft_core::types::policy::COORD_ABS_MAX as f32
        && point.1.abs() <= paintcraft_core::types::policy::COORD_ABS_MAX as f32
}
fn moved<S>(drag: &Drag<S>, points_per_pixel: f32) -> bool {
    points_per_pixel.is_finite()
        && points_per_pixel > 0.0
        && (drag.to.0 - drag.from.0).hypot(drag.to.1 - drag.from.1) * points_per_pixel >= 4.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jitter_is_a_click_and_boxes_keep_subpixel_geometry_in_either_direction() {
        let mut gesture = ZoomGesture::default();
        gesture.begin(1, PointerButton::Primary, (10.25, 20.5));
        assert_eq!(
            gesture.release(&1, PointerButton::Primary, (11.0, 21.0), 1.0),
            Some(ZoomRequest::Step {
                center: (10.25, 20.5),
                increase: true
            })
        );
        gesture.begin(1, PointerButton::Primary, (40.75, 60.25));
        gesture.update((10.25, 20.5));
        assert_eq!(gesture.preview(1.0), Some(((40.75, 60.25), (10.25, 20.5))));
        assert_eq!(
            gesture.release(&1, PointerButton::Primary, (10.25, 20.5), 1.0),
            Some(ZoomRequest::Rectangle {
                from: (40.75, 60.25),
                to: (10.25, 20.5)
            })
        );
        assert!(!gesture.is_active());
    }
    #[test]
    fn stale_sources_cancel_and_foreign_buttons_cannot_finish_or_replace_owner() {
        let mut gesture = ZoomGesture::default();
        gesture.begin(1, PointerButton::Primary, (10.0, 20.0));
        gesture.begin(1, PointerButton::Secondary, (40.0, 50.0));
        assert_eq!(
            gesture.release(&1, PointerButton::Secondary, (80.0, 90.0), 1.0),
            None
        );
        assert!(gesture.is_active());
        assert_eq!(
            gesture.release(&2, PointerButton::Primary, (80.0, 90.0), 1.0),
            None
        );
        assert!(!gesture.is_active());
        gesture.begin(2, PointerButton::Primary, (0.0, 0.0));
        gesture.validate(&2, false);
        assert!(!gesture.is_active());
    }
    #[test]
    fn secondary_drag_is_one_zoom_out_and_invalid_coordinates_cannot_publish() {
        let mut gesture = ZoomGesture::default();
        gesture.begin(1, PointerButton::Secondary, (1.0, 2.0));
        gesture.update((100.0, 200.0));
        assert!(gesture.preview(1.0).is_none());
        assert_eq!(
            gesture.release(&1, PointerButton::Secondary, (100.0, 200.0), 1.0),
            Some(ZoomRequest::Step {
                center: (1.0, 2.0),
                increase: false
            })
        );
        gesture.begin(1, PointerButton::Primary, (f32::NAN, 0.0));
        assert!(!gesture.is_active());
        gesture.begin(1, PointerButton::Primary, (0.0, 0.0));
        assert!(
            gesture
                .release(&1, PointerButton::Primary, (f32::INFINITY, 0.0), 1.0)
                .is_none()
        );
        assert!(!gesture.is_active());
    }
}
