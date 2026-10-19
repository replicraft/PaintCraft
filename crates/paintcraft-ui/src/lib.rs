//! Everything the desktop application decides, without a window or egui.
//! The application feeds pointer samples and button presses in; this crate
//! turns them into at most one transaction per user intention and reports
//! state back. Keeping it headless is what makes "one click, one edit" testable.

pub mod editor;
pub mod outbox;
pub mod settings;
pub mod view;
