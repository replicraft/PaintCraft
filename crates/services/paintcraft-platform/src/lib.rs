//! OS services are independent of documents, sessions and frontend widgets.
#[cfg(feature = "dialogs")]
pub mod dialogs;
pub mod save;

#[cfg(all(feature = "menus", target_os = "macos"))]
pub mod native_menu;

#[cfg(feature = "fonts")]
pub mod fonts;

pub mod spill;
