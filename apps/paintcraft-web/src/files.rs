//! Browser pickers and download dialogs. Downloads never claim confirmed disk saves.
#[cfg(any(target_arch = "wasm32", test))]
use crate::editor::MAX_PIXELS;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_formats::{image, pcnd};
#[cfg(any(target_arch = "wasm32", test))]
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc};

#[cfg(any(target_arch = "wasm32", test))]
const FILE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct Loaded {
    pub document: Document,
    pub name: String,
    pub losses: Vec<String>,
}
#[derive(Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) enum Outcome {
    Loaded(Loaded),
    Cancelled,
    DownloadClosed,
}

#[derive(Debug, Default)]
pub(crate) struct Files {
    pub busy: bool,
    outcome: Rc<RefCell<Option<Result<Outcome>>>>,
}

impl Files {
    pub fn poll(&mut self) -> Option<Result<Outcome>> {
        let result = self.outcome.borrow_mut().take();
        if result.is_some() {
            self.busy = false;
        }
        result
    }

    #[cfg(target_arch = "wasm32")]
    pub fn open(&mut self, ctx: &eframe::egui::Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        let target = Rc::clone(&self.outcome);
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title("Open PaintCraft document or image")
                    .add_filter(
                        "PaintCraft and images",
                        &["pcnd", "png", "bmp", "jpg", "jpeg"],
                    )
                    .pick_file()
                    .await
                else {
                    return Ok(Outcome::Cancelled);
                };
                if file.inner().size() > FILE_BYTES as f64 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "Browser input limit is 64 MiB",
                    ));
                }
                // FileHandle::read unwraps read failures; use the fallible browser API.
                let buffer = wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer())
                    .await
                    .map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Browser file read failed")
                    })?;
                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                Ok(Outcome::Loaded(decode(&bytes, file.file_name())?))
            }
            .await;
            *target.borrow_mut() = Some(result);
            focus_canvas();
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&mut self, _ctx: &eframe::egui::Context) {
        *self.outcome.borrow_mut() = Some(Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "File picking is available in the browser build",
        )));
    }

    #[cfg(target_arch = "wasm32")]
    pub fn download(&mut self, ctx: &eframe::egui::Context, document: &Document, png: bool) {
        if self.busy {
            return;
        }
        let bytes = match encode(document, png) {
            Ok(bytes) => bytes,
            Err(error) => {
                *self.outcome.borrow_mut() = Some(Err(error));
                return;
            }
        };
        self.busy = true;
        let target = Rc::clone(&self.outcome);
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let name = if png { "Untitled.png" } else { "Untitled.pcnd" };
            let result = wasm_bindgen_futures::JsFuture::from(download_copy(&bytes, name))
                .await
                .map(|requested| {
                    if requested.as_bool() == Some(true) {
                        Outcome::DownloadClosed
                    } else {
                        Outcome::Cancelled
                    }
                })
                .map_err(|_| {
                    Error::new(ErrorCode::StorageFailed, "Browser download dialog failed")
                });
            *target.borrow_mut() = Some(result);
            focus_canvas();
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn download(&mut self, _ctx: &eframe::egui::Context, _document: &Document, _png: bool) {
        *self.outcome.borrow_mut() = Some(Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Downloads are available in the browser build",
        )));
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn decode(bytes: &[u8], name: String) -> Result<Loaded> {
    if bytes.len() > FILE_BYTES {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Browser input limit is 64 MiB",
        ));
    }
    if bytes.starts_with(b"PK") {
        return Ok(Loaded {
            document: pcnd::read_with_limits(bytes, MAX_PIXELS, MAX_PIXELS * 4)?,
            name,
            losses: Vec::new(),
        });
    }
    let image = image::decode_with_pixel_limit(bytes, MAX_PIXELS)?;
    let sha = paintcraft_document::hex(&Sha256::digest(bytes));
    Ok(Loaded {
        document: paintcraft_document::import_image(image.width, image.height, &image.rgba, &sha)?,
        name,
        losses: image.losses,
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn encode(document: &Document, png: bool) -> Result<Vec<u8>> {
    if png {
        return Ok(paintcraft_formats::export::export_flat(
            document,
            image::ImageFormat::Png,
            90,
            &["flatten".into(), "hidden-rgb".into()],
        )?
        .bytes);
    }
    let mut writer = Limited(Vec::new());
    pcnd::write_to(document, &mut writer)?;
    Ok(writer.0)
}

#[cfg(any(target_arch = "wasm32", test))]
struct Limited(Vec<u8>);
#[cfg(any(target_arch = "wasm32", test))]
impl std::io::Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > FILE_BYTES {
            return Err(std::io::ErrorKind::FileTooLarge.into());
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| std::io::ErrorKind::OutOfMemory)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Editor, WHITE};
    #[test]
    fn project_preserves_layers_and_png_preserves_flat_pixels() {
        let mut editor = Editor::new(24, 16).unwrap();
        editor
            .apply(paintcraft_tools::Action::Fill {
                layer: editor.target.unwrap(),
                at: (0, 0),
                color: paintcraft_core::types::Rgba([20, 80, 140, 255]),
                tolerance: 0,
            })
            .unwrap();
        editor
            .commit(vec![paintcraft_core::types::json::Value::object([
                ("op", paintcraft_core::types::json::Value::str("layer.add")),
                ("ref", paintcraft_core::types::json::Value::str("new")),
                ("name", paintcraft_core::types::json::Value::str("Overlay")),
                ("width", paintcraft_core::types::json::Value::Int(24)),
                ("height", paintcraft_core::types::json::Value::Int(16)),
                (
                    "origin",
                    paintcraft_core::types::json::Value::object([
                        ("x", paintcraft_core::types::json::Value::Int(0)),
                        ("y", paintcraft_core::types::json::Value::Int(0)),
                    ]),
                ),
                (
                    "default_pixel",
                    paintcraft_core::types::json::Value::str("#FF000080"),
                ),
            ])])
            .unwrap();
        let doc = editor.session.document();
        let bytes = encode(doc, false).unwrap();
        let loaded = decode(&bytes, "example.pcnd".into()).unwrap();
        assert!(loaded.document.same_semantics(doc));
        assert!(loaded.losses.is_empty());
        let png = encode(doc, true).unwrap();
        let loaded = decode(&png, "example.png".into()).unwrap();
        assert_eq!(loaded.document.canvas, doc.canvas);
        assert_eq!(
            paintcraft_render::render_flat(&loaded.document).unwrap(),
            paintcraft_render::render_flat(doc).unwrap()
        );
        assert_eq!(
            loaded
                .document
                .layers_in_order()
                .next()
                .unwrap()
                .raster
                .default_pixel(),
            paintcraft_core::types::Rgba::TRANSPARENT
        );
        assert_eq!(WHITE.0, [255; 4]);
    }
    #[test]
    fn rejected_file_never_replaces_the_current_session() {
        let editor = Editor::new(16, 16).unwrap();
        let before = editor.session.ticket();
        assert!(decode(b"bad file", "example.png".into()).is_err());
        let large = paintcraft_document::new_document(
            4096,
            4096,
            Some(WHITE),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        assert_eq!(
            decode(&pcnd::write(&large).unwrap(), "large.pcnd".into())
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(editor.session.ticket(), before);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/download.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = downloadCopy)]
    fn download_copy(bytes: &[u8], name: &str) -> js_sys::Promise;
}

#[cfg(target_arch = "wasm32")]
fn focus_canvas() {
    use wasm_bindgen::JsCast;
    if let Some(canvas) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("paintcraft"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = canvas.focus();
    }
}
