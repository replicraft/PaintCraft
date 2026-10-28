//! Canonical tracking belongs to this exact public area, never an inferred
//! private winit tracking-rectangle tag or an unrelated view in the window.
use super::*;
use objc2::AllocAnyThread;
use objc2_app_kit::{NSTrackingArea, NSTrackingAreaOptions};

pub(super) struct Tracking {
    view: Retained<NSView>,
    area: Retained<NSTrackingArea>,
    attached: bool,
}
impl Tracking {
    pub(super) fn new(view: &NSView) -> Self {
        // SAFETY: NSView is the retained main-thread event recipient and
        // implements mouseEntered:/mouseExited:. No private class is changed.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                view.bounds(),
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveInKeyWindow
                    | NSTrackingAreaOptions::InVisibleRect
                    | NSTrackingAreaOptions::EnabledDuringMouseDrag,
                Some(view.as_ref()),
                None,
            )
        };
        Self {
            view: view.retain(),
            area,
            attached: false,
        }
    }
    pub(super) fn area(&self) -> &NSTrackingArea {
        &self.area
    }
    pub(super) fn attach(&mut self) {
        // Install the monitor first so synchronous entry observations cannot
        // precede source observation. Geometry follows the visible view.
        self.view.addTrackingArea(&self.area);
        self.attached = true;
    }
}
impl Drop for Tracking {
    fn drop(&mut self) {
        if self.attached {
            self.view.removeTrackingArea(&self.area);
        }
    }
}

pub(super) fn is_ours(event: &NSEvent, expected: Option<&NSTrackingArea>) -> bool {
    if !matches!(
        event.r#type(),
        NSEventType::MouseEntered | NSEventType::MouseExited
    ) {
        return false;
    }
    let Some(expected) = expected else {
        return false;
    };
    // Only tracking-event kinds may access this property. Legacy rectangles
    // return None; a same-window event is not proof of our area identity.
    event
        .trackingArea()
        .is_some_and(|actual| std::ptr::eq(actual.as_ref(), expected))
}
