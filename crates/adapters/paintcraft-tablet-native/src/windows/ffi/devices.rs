//! Notifications have no unregister function. A private message-only HWND
//! bounds their registration lifetime without changing process-wide raw input.
use super::*;
use windows::{Win32::UI::Controls::RegisterPointerDeviceNotifications, core::w};

pub(super) struct Notifications {
    hwnd: HWND,
    context: *mut Context,
}
impl Notifications {
    pub fn new(inner: &Rc<Inner>, installation: u64) -> Result<Self> {
        // SAFETY: predefined system STATIC class, no visible style, owning
        // thread; HWND_MESSAGE is a message-only parent, never a shown window.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("PaintCraft tablet notifications"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map_err(|_| error("Pointer notification window creation failed"))?;
        let context = Box::into_raw(Box::new(Context {
            inner: inner.clone(),
            installation,
        }));
        let owner = Self { hwnd, context };
        // SAFETY: private stable refdata and callback on the owning thread.
        if !unsafe {
            SetWindowSubclass(
                hwnd,
                Some(lifecycle::subclass),
                context as usize,
                context as usize,
            )
        }
        .as_bool()
        {
            return Err(error("Pointer notification observer installation failed"));
        }
        unsafe { RegisterPointerDeviceNotifications(hwnd, true) }
            .map_err(|_| error("Pointer device notifications unavailable"))?;
        Ok(owner)
    }
}
impl Drop for Notifications {
    fn drop(&mut self) {
        // SAFETY: !Send owner is retained by Installation on the owning thread;
        // destruction retires registration and all HWND subclass callbacks.
        if unsafe { DestroyWindow(self.hwnd) }.is_ok()
            || !unsafe { IsWindow(Some(self.hwnd)) }.as_bool()
        {
            unsafe {
                drop(Box::from_raw(self.context));
            }
        } else {
            // This Hook cannot accumulate more retained contexts on retry.
            // SAFETY: failed removal leaves this stable Box owned by Windows.
            unsafe { &*self.context }.inner.retirement_failed.set(true);
        }
        // If native removal fails, retain refdata; freeing a callable pointer
        // would be unsafe. The disabled Hook prevents subsequent acquisition.
    }
}
