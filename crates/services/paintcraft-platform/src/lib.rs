//! OS services are independent of documents, sessions and frontend widgets.
#[cfg(feature = "dialogs")]
pub mod dialogs;
pub mod save;

#[cfg(all(feature = "menus", target_os = "macos"))]
pub mod native_menu;

#[cfg(feature = "fonts")]
pub mod fonts;

pub mod spill;

#[cfg(feature = "printing")]
pub mod printing;

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(not(target_arch = "wasm32"))]
pub mod source;

#[cfg(not(target_arch = "wasm32"))]
pub mod codec_helper;
