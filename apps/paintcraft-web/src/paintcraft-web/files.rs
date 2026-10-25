//! Browser pickers and immediate downloads. Downloads never claim confirmed disk saves.
#[cfg(any(target_arch = "wasm32", test))]
use crate::editor::MAX_PIXELS;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_formats::registry::Frontend;
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_formats::registry::{Family, Platform};
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_formats::{image, pcnd};
use paintcraft_ui::actions::ImageExport;
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
    DownloadRequested {
        owner: uuid::Uuid,
        project_name: Option<String>,
    },
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
    pub fn open(&mut self, ctx: &eframe::egui::Context, frontend: Frontend) {
        if self.busy {
            return;
        }
        self.busy = true;
        let target = Rc::clone(&self.outcome);
        let ctx = ctx.clone();
        let extensions = paintcraft_formats::registry::enabled_extensions(
            frontend,
            paintcraft_formats::registry::Operation::Open,
            Platform::Web,
        );
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title("Open PaintCraft document or image")
                    .add_filter("PaintCraft and images", &extensions)
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
                Ok(Outcome::Loaded(decode_for_profile(
                    &bytes,
                    file.file_name(),
                    frontend,
                )?))
            }
            .await;
            *target.borrow_mut() = Some(result);
            focus_canvas();
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&mut self, _ctx: &eframe::egui::Context, _frontend: Frontend) {
        *self.outcome.borrow_mut() = Some(Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "File picking is available in the browser build",
        )));
    }

    #[cfg(target_arch = "wasm32")]
    pub fn download(
        &mut self,
        ctx: &eframe::egui::Context,
        document: &Document,
        format: Option<ImageExport>,
        jpeg_quality: u8,
        name: &str,
        owner: uuid::Uuid,
    ) {
        if self.busy {
            return;
        }
        let bytes = match encode_quality(document, format, jpeg_quality) {
            Ok(bytes) => bytes,
            Err(error) => {
                *self.outcome.borrow_mut() = Some(Err(error));
                return;
            }
        };
        self.busy = true;
        let mime = match format {
            None => "application/zip",
            Some(ImageExport::Png) => "image/png",
            Some(ImageExport::Jpeg) => "image/jpeg",
            Some(ImageExport::Bmp) => "image/bmp",
        };
        *self.outcome.borrow_mut() = Some(
            download_copy(&bytes, name, mime)
                .map(|()| Outcome::DownloadRequested {
                    owner,
                    project_name: format.is_none().then(|| name.to_owned()),
                })
                .map_err(|_| Error::new(ErrorCode::StorageFailed, "Browser download failed")),
        );
        focus_canvas();
        ctx.request_repaint();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn download(
        &mut self,
        _ctx: &eframe::egui::Context,
        _document: &Document,
        _format: Option<ImageExport>,
        _jpeg_quality: u8,
        _name: &str,
        _owner: uuid::Uuid,
    ) {
        *self.outcome.borrow_mut() = Some(Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Downloads are available in the browser build",
        )));
    }
}

#[cfg(test)]
fn decode(bytes: &[u8], name: String) -> Result<Loaded> {
    decode_for_profile(bytes, name, Frontend::ClassicV11)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn decode_for_profile(bytes: &[u8], name: String, frontend: Frontend) -> Result<Loaded> {
    if bytes.len() > FILE_BYTES {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Browser input limit is 64 MiB",
        ));
    }
    let family = paintcraft_formats::registry::admit_open(frontend, &name, bytes, Platform::Web)?;
    if family == Family::Pcnd {
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

#[cfg(test)]
fn encode(document: &Document, format: Option<ImageExport>) -> Result<Vec<u8>> {
    encode_quality(document, format, 90)
}

#[cfg(any(target_arch = "wasm32", test))]
fn encode_quality(
    document: &Document,
    format: Option<ImageExport>,
    jpeg_quality: u8,
) -> Result<Vec<u8>> {
    if let Some(format) = format {
        let format = match format {
            ImageExport::Png => image::ImageFormat::Png,
            ImageExport::Jpeg => image::ImageFormat::Jpeg,
            ImageExport::Bmp => image::ImageFormat::Bmp,
        };
        // Every currently exposed browser export is shared by all Classic skins.
        // Any future addition must capture the active profile before widening this enum.
        use paintcraft_formats::registry::{self, Operation, Request};
        let family = Family::from_extension(format.name()).ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Unregistered export format",
            )
        })?;
        let request = Request::new(
            Frontend::ClassicV11,
            Operation::ExportFlat,
            family,
            Platform::Web,
        );
        let decision = registry::admission(request);
        if !decision.available() {
            decision.require_allowed(request)?;
        }
        return Ok(paintcraft_formats::export::export_flat(
            document,
            format,
            jpeg_quality,
            &paintcraft_formats::export::KNOWN_LOSSES.map(str::to_owned),
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
    fn frozen_frontend_profile_controls_browser_decode_before_import() {
        let png = image::encode(image::ImageFormat::Png, 1, 1, &[12, 34, 56, 0], 90).unwrap();
        for frontend in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            let loaded = decode_for_profile(&png, "image.png".into(), frontend).unwrap();
            assert_eq!(loaded.document.canvas, (1, 1));
            for name in [
                "fake.svg",
                "fake.pdn",
                "fake.psd",
                "mismatch.jpg",
                "unknown.xyz",
            ] {
                assert!(
                    decode_for_profile(&png, name.into(), frontend).is_err(),
                    "{frontend:?}/{name}"
                );
            }
            assert!(decode_for_profile(b"PK\x03\x04", "unknown.zip".into(), frontend).is_err());
            assert!(decode_for_profile(b"PK\x03\x04", "invalid.pcnd".into(), frontend).is_err());
        }
        // The captured argument, rather than later preferences, is the authority.
        let frontend = Frontend::ClassicV7;
        assert!(
            decode_for_profile(&png, "fake.webp".into(), frontend)
                .unwrap_err()
                .message
                .contains("ClassicV7")
        );
    }
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
        let bytes = encode(doc, None).unwrap();
        let loaded = decode(&bytes, "example.pcnd".into()).unwrap();
        assert!(loaded.document.same_semantics(doc));
        assert!(loaded.losses.is_empty());
        let png = encode(doc, Some(ImageExport::Png)).unwrap();
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
    fn image_export_formats_roundtrip_with_explicit_jpeg_losses_without_publication() {
        let document = paintcraft_document::new_document(
            24,
            16,
            Some(paintcraft_core::types::Rgba([40, 80, 140, 128])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let state = document.state_id;
        for format in ImageExport::ALL {
            let bytes = encode(&document, Some(format)).unwrap();
            let magic: &[u8] = match format {
                ImageExport::Png => b"\x89PNG\r\n\x1a\n",
                ImageExport::Jpeg => &[255, 216, 255],
                ImageExport::Bmp => b"BM",
            };
            assert!(bytes.starts_with(magic));
            let decoded = image::decode(&bytes).unwrap();
            assert_eq!((decoded.width, decoded.height), (24, 16));
            assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|pixel| pixel[3]
                == if format == ImageExport::Jpeg {
                    255
                } else {
                    128
                }));
            assert_eq!(document.state_id, state);
        }
    }
    #[test]
    fn jpeg_quality_changes_the_downloaded_codestream_but_preserves_the_project() {
        let document = paintcraft_document::new_document(
            32,
            24,
            Some(paintcraft_core::types::Rgba([40, 80, 140, 128])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let snapshot = document.clone();
        let low = encode_quality(&document, Some(ImageExport::Jpeg), 10).unwrap();
        let high = encode_quality(&document, Some(ImageExport::Jpeg), 95).unwrap();
        assert_ne!(low, high, "quality must reach the actual JPEG encoder");
        for bytes in [&low, &high] {
            let decoded = image::decode(bytes).unwrap();
            assert_eq!((decoded.width, decoded.height), document.canvas);
            // The exported alpha is composed over white before JPEG encoding.
            for pixel in decoded.rgba.as_chunks::<4>().0 {
                assert_eq!(pixel[3], 255);
                for (actual, expected) in pixel[..3].iter().zip([147, 167, 197]) {
                    assert!((i16::from(*actual) - expected).abs() <= 12);
                }
            }
        }
        for format in [None, Some(ImageExport::Png), Some(ImageExport::Bmp)] {
            assert_eq!(
                encode_quality(&document, format, 10).unwrap(),
                encode_quality(&document, format, 95).unwrap()
            );
        }
        assert!(document.same_semantics(&snapshot));
        assert_eq!(document.state_id, snapshot.state_id);
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
#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/lib/download.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = downloadCopy)]
    fn download_copy(
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> std::result::Result<(), wasm_bindgen::JsValue>;
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
