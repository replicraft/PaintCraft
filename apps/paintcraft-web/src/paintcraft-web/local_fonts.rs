//! Permission-gated browser fonts, shared across the independent workspaces.
use eframe::egui;
use paintcraft_tools::text::TextRenderer;
use paintcraft_ui::t;

/// Request from the Text preferences navigation click, while activation is live.
pub(crate) fn request_access(ctx: &egui::Context) {
    #[cfg(target_arch = "wasm32")]
    browser::enumerate(ctx, true);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = ctx;
}

pub(crate) fn controls(ui: &mut egui::Ui) {
    #[cfg(target_arch = "wasm32")]
    {
        if browser::local_fonts_supported() {
            if ui.button(t!(ui.ctx(), "Load installed fonts")).clicked() {
                browser::enumerate(ui.ctx(), true);
            }
            browser::STATE.with_borrow(|state| {
                if let Some(status) = state.status {
                    ui.weak(t!(ui.ctx(), status));
                }
            });
        } else {
            ui.weak(t!(
                ui.ctx(),
                "This browser does not support installed font access. Bundled fonts are available."
            ));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    ui.weak(t!(
        ui.ctx(),
        "This browser does not support installed font access. Bundled fonts are available."
    ));
}

pub(crate) fn refresh(ctx: &egui::Context, family: &str, families: &mut Vec<String>) {
    #[cfg(target_arch = "wasm32")]
    {
        let first = browser::STATE.with_borrow_mut(|state| {
            let first = !state.started;
            state.started = true;
            first
        });
        if first {
            browser::enumerate(ctx, false);
        }
        browser::ensure(ctx, family);
        browser::STATE.with_borrow(|state| {
            if !state.families.is_empty() && *families != state.families {
                families.clone_from(&state.families);
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (ctx, family, families);
}

pub(crate) fn generation() -> u64 {
    #[cfg(target_arch = "wasm32")]
    return browser::STATE.with_borrow(|state| state.generation);
    #[cfg(not(target_arch = "wasm32"))]
    0
}

pub(crate) fn prepare(
    renderer: &mut TextRenderer,
    family: &str,
) -> paintcraft_core::types::Result<()> {
    #[cfg(target_arch = "wasm32")]
    return browser::STATE.with_borrow(|state| {
        if state.names.iter().any(|name| name == family) {
            let Some(bytes) = state.loaded.get(family) else {
                return Err(paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::InputSchema,
                    "The selected font is still loading or could not be loaded",
                ));
            };
            if !renderer
                .database()
                .faces()
                .any(|face| face.families.iter().any(|(name, _)| name == family))
            {
                for data in bytes {
                    renderer.load_font_data(data.clone());
                }
            }
        }
        Ok(())
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (renderer, family);
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use eframe::egui;
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
    };
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    #[derive(Default)]
    pub(super) struct State {
        pub started: bool,
        pub generation: u64,
        busy: bool,
        pub names: Vec<String>,
        pub families: Vec<String>,
        pub loaded: BTreeMap<String, Vec<Vec<u8>>>,
        pending: BTreeSet<String>,
        pub status: Option<&'static str>,
    }
    thread_local! { pub(super) static STATE: RefCell<State> = RefCell::default(); }
    #[wasm_bindgen(module = "/src/lib/local-fonts.js")]
    unsafe extern "C" {
        #[wasm_bindgen(js_name = localFontsSupported)]
        pub fn local_fonts_supported() -> bool;
        #[wasm_bindgen(js_name = listLocalFonts)]
        fn list_local_fonts(request: bool) -> js_sys::Promise;
        #[wasm_bindgen(js_name = loadLocalFamily)]
        fn load_local_family(family: &str) -> js_sys::Promise;
    }
    pub fn enumerate(ctx: &egui::Context, request: bool) {
        if !local_fonts_supported()
            || STATE.with_borrow_mut(|s| std::mem::replace(&mut s.busy, true))
        {
            return;
        }
        let promise = list_local_fonts(request);
        let ctx = ctx.clone();
        spawn_local(async move {
            let result = JsFuture::from(promise).await;
            STATE.with_borrow_mut(|state| {
                state.busy = false;
                match result {
                    Ok(value) => {
                        state.names = js_sys::Array::from(&value)
                            .iter()
                            .filter_map(|v| v.as_string())
                            .filter(|s| !s.is_empty() && s.len() <= 256)
                            .collect();
                        state.families = paintcraft_ui::fonts::BUNDLED_FAMILIES
                            .iter()
                            .map(|s| (*s).into())
                            .chain(state.names.iter().cloned())
                            .collect();
                        state.families.sort();
                        state.families.dedup();
                        state.status = Some(if state.names.is_empty() {
                            "Allow font access to use installed fonts."
                        } else {
                            "Installed fonts are available."
                        });
                    }
                    Err(_) => {
                        state.status =
                            Some("Font access was denied or is unavailable. You can try again.")
                    }
                }
            });
            ctx.request_repaint();
        });
    }
    pub fn ensure(ctx: &egui::Context, family: &str) {
        let needed = STATE.with_borrow_mut(|s| {
            s.names.iter().any(|n| n == family)
                && !s.loaded.contains_key(family)
                && s.pending.insert(family.into())
        });
        if !needed {
            return;
        }
        let promise = load_local_family(family);
        let family = family.to_owned();
        let ctx = ctx.clone();
        spawn_local(async move {
            let result = JsFuture::from(promise).await;
            STATE.with_borrow_mut(|s| match result {
                Ok(value) => {
                    let faces = js_sys::Array::from(&value).iter()
                        .map(|v| js_sys::Uint8Array::new(&v).to_vec()).collect();
                    s.loaded.insert(family, faces);
                    s.generation += 1;
                }
                Err(_) => {
                    s.status = Some("The selected font could not be loaded. Choose another font or reload to try again.");
                }
            });
            ctx.request_repaint();
        });
    }
}
