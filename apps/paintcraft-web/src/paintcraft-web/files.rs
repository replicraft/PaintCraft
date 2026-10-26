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
    pub converted_copy: bool,
    pub document: Document,
    pub name: String,
    pub losses: Vec<String>,
}
#[derive(Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) enum Outcome {
    Loaded(Loaded),
    Pages(crate::importing::pages::Source),
    Svg(crate::importing::svg::Source),
    Cancelled,
    DownloadRequested {
        owner: uuid::Uuid,
        project_name: Option<String>,
    },
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) struct ExportOptions {
    pub format: Option<ImageExport>,
    pub jpeg_quality: u8,
    pub pcx_encoding: paintcraft_ui::pcx_export::Encoding,
    pub frontend: Frontend,
}

#[derive(Debug, Default)]
pub(crate) struct Files {
    pub busy: bool,
    outcome: Rc<RefCell<Option<Result<Outcome>>>>,
}

impl Files {
    pub fn download_asset(
        &mut self,
        ctx: &eframe::egui::Context,
        bytes: &[u8],
        name: &str,
        owner: uuid::Uuid,
    ) {
        if self.busy {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let result = download_copy(bytes, name, "image/x-icon")
            .map(|()| Outcome::DownloadRequested {
                owner,
                project_name: None,
            })
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "Browser download failed"));
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            let _ = (bytes, name, owner);
            Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Downloads are available in the browser build",
            ))
        };
        self.busy = true;
        *self.outcome.borrow_mut() = Some(result);
        ctx.request_repaint();
    }
    pub fn poll(&mut self) -> Option<Result<Outcome>> {
        let result = self.outcome.borrow_mut().take();
        if result.is_some() {
            self.busy = false;
        }
        result
    }

    #[cfg(target_arch = "wasm32")]
    pub fn open(&mut self, ctx: &eframe::egui::Context, frontend: Frontend, convert: bool) {
        if self.busy {
            return;
        }
        self.busy = true;
        let target = Rc::clone(&self.outcome);
        let ctx = ctx.clone();
        let mut extensions: Vec<_> = paintcraft_formats::registry::enabled_extensions(
            frontend,
            if convert {
                paintcraft_formats::registry::Operation::ImportConvert
            } else {
                paintcraft_formats::registry::Operation::Open
            },
            Platform::Web,
        )
        .into_iter()
        .filter(|extension| {
            !convert
                || Family::from_extension(extension).is_some_and(|family| {
                    !family.is_project() || paintcraft_formats::project_import::supports(family)
                })
        })
        .collect();
        if convert && frontend == Frontend::Studio {
            extensions.push("dcx");
            extensions.extend(["ico", "cur"]);
            extensions.extend(["svg", "svgz"]);
        }
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
                let name = file.file_name();
                let svg = convert
                    && name.rsplit('.').next().and_then(Family::from_extension)
                        == Some(Family::Svg);
                if svg && frontend != Frontend::Studio {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "SVG rasterization requires explicit Studio import",
                    ));
                }
                let allowance = if svg {
                    paintcraft_formats::svg::SOURCE_MAX
                } else {
                    FILE_BYTES
                };
                if file.inner().size() > allowance as f64 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        format!("Browser input exceeds {allowance} bytes"),
                    ));
                }
                // FileHandle::read unwraps read failures; use the fallible browser API.
                let buffer = wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer())
                    .await
                    .map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Browser file read failed")
                    })?;
                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                if svg {
                    return Ok(Outcome::Svg(crate::importing::svg::Source::new(
                        bytes, name, frontend,
                    )?));
                }
                if convert
                    && matches!(
                        paintcraft_formats::registry::sniff(&bytes),
                        Some(Family::Dcx | Family::Ico | Family::Cur)
                    )
                {
                    return Ok(Outcome::Pages(crate::importing::pages::Source::new(
                        bytes,
                        file.file_name(),
                        frontend,
                    )?));
                }
                Ok(Outcome::Loaded(decode_input(
                    &bytes,
                    file.file_name(),
                    frontend,
                    convert,
                )?))
            }
            .await;
            *target.borrow_mut() = Some(result);
            focus_canvas();
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&mut self, _ctx: &eframe::egui::Context, _frontend: Frontend, _convert: bool) {
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
        options: ExportOptions,
        name: &str,
        owner: uuid::Uuid,
    ) {
        if self.busy {
            return;
        }
        let bytes = match encode_options_for_profile(
            document,
            options.format,
            options.jpeg_quality,
            options.frontend,
            options.pcx_encoding,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                *self.outcome.borrow_mut() = Some(Err(error));
                return;
            }
        };
        self.busy = true;
        let mime = options
            .format
            .map_or("application/x-paintcraft", ImageExport::mime);
        *self.outcome.borrow_mut() = Some(
            download_copy(&bytes, name, mime)
                .map(|()| Outcome::DownloadRequested {
                    owner,
                    project_name: options.format.is_none().then(|| name.to_owned()),
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
        _options: ExportOptions,
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

#[cfg(test)]
pub(crate) fn decode_for_profile(bytes: &[u8], name: String, frontend: Frontend) -> Result<Loaded> {
    decode_input(bytes, name, frontend, false)
}

#[cfg(any(target_arch = "wasm32", test))]
fn decode_input(bytes: &[u8], name: String, frontend: Frontend, convert: bool) -> Result<Loaded> {
    if bytes.len() > FILE_BYTES {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Browser input limit is 64 MiB",
        ));
    }
    let family = if convert {
        paintcraft_formats::registry::admit_import(frontend, &name, bytes, Platform::Web)?
    } else {
        paintcraft_formats::registry::admit_open(frontend, &name, bytes, Platform::Web)?
    };
    if convert && family.is_project() && !paintcraft_formats::project_import::supports(family) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Import image expects a raster picture",
        ));
    }
    if paintcraft_formats::project_import::supports(family) {
        let converted =
            paintcraft_formats::project_import::decode_layered(family, bytes, MAX_PIXELS, &|| {
                Ok(())
            })?;
        return Ok(Loaded {
            converted_copy: true,
            document: converted.document,
            name,
            losses: converted.losses,
        });
    }
    if family == Family::Pcnd {
        return Ok(Loaded {
            converted_copy: false,
            document: pcnd::read_with_limits(bytes, MAX_PIXELS, MAX_PIXELS * 4)?,
            name,
            losses: Vec::new(),
        });
    }
    let image = image::decode_with_pixel_limit(bytes, MAX_PIXELS)?;
    let sha = paintcraft_document::hex(&Sha256::digest(bytes));
    Ok(Loaded {
        converted_copy: convert,
        document: paintcraft_document::import_image(image.width, image.height, &image.rgba, &sha)?,
        name,
        losses: image.losses,
    })
}

#[cfg(test)]
fn encode(document: &Document, format: Option<ImageExport>) -> Result<Vec<u8>> {
    encode_quality(document, format, 90)
}

#[cfg(test)]
fn encode_quality(
    document: &Document,
    format: Option<ImageExport>,
    jpeg_quality: u8,
) -> Result<Vec<u8>> {
    encode_quality_for_profile(document, format, jpeg_quality, Frontend::Studio)
}

#[cfg(any(target_arch = "wasm32", test))]
fn encode_options_for_profile(
    document: &Document,
    format: Option<ImageExport>,
    jpeg_quality: u8,
    frontend: Frontend,
    pcx_encoding: paintcraft_ui::pcx_export::Encoding,
) -> Result<Vec<u8>> {
    if let Some(format) = format {
        let format = match format {
            ImageExport::Png => image::ImageFormat::Png,
            ImageExport::Jpeg => image::ImageFormat::Jpeg,
            ImageExport::Bmp => image::ImageFormat::Bmp,
            ImageExport::Gif => image::ImageFormat::Gif,
            ImageExport::Tiff => image::ImageFormat::Tiff,
            ImageExport::Tga => image::ImageFormat::Tga,
            ImageExport::Webp => image::ImageFormat::WebP,
            ImageExport::Jxl => image::ImageFormat::Jxl,
            ImageExport::Dds => image::ImageFormat::Dds,
            ImageExport::Avif => image::ImageFormat::Avif,
            ImageExport::Qoi => image::ImageFormat::Qoi,
            ImageExport::Pam => image::ImageFormat::Pnm,
            ImageExport::Farbfeld => image::ImageFormat::Farbfeld,
            ImageExport::Pcx => image::ImageFormat::Pcx,
        };
        // The captured frontend determines admission before rendering or encoding.
        use paintcraft_formats::registry::{self, Operation, Request};
        let family = Family::from_id(format.name()).ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Unregistered export format",
            )
        })?;
        let request = Request::new(frontend, Operation::ExportFlat, family, Platform::Web);
        let decision = registry::admission(request);
        if !decision.available() {
            decision.require_allowed(request)?;
        }
        let mut options = paintcraft_formats::export::FlatOptions::new(format, jpeg_quality);
        if format == image::ImageFormat::Pcx {
            options.pcx_mode = pcx_mode(pcx_encoding);
        }
        return Ok(paintcraft_formats::export::export_flat_options(
            document,
            options,
            &paintcraft_formats::export::KNOWN_LOSSES.map(str::to_owned),
        )?
        .bytes);
    }
    let mut writer = Limited(Vec::new());
    pcnd::write_compact_to(document, &mut writer)?;
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
        for format in ImageExport::ALL
            .into_iter()
            .filter(|format| !matches!(format, ImageExport::Jxl | ImageExport::Avif))
        {
            let bytes = encode(&document, Some(format)).unwrap();
            assert_eq!(
                image::ImageFormat::sniff(&bytes).unwrap().name(),
                if format == ImageExport::Pam {
                    "PNM"
                } else {
                    format.name()
                }
            );
            let decoded = image::decode(&bytes).unwrap();
            assert_eq!((decoded.width, decoded.height), (24, 16));
            assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|pixel| pixel[3]
                == if matches!(
                    format,
                    ImageExport::Jpeg | ImageExport::Gif | ImageExport::Pcx
                ) {
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
    pub(crate) fn download_copy(
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

#[cfg(test)]
mod expanded_profiles {
    use super::*;
    #[test]
    fn studio_exports_keep_the_captured_profile_and_classic_cannot_gain_stock_studio_formats() {
        let document = paintcraft_document::new_document(
            3,
            2,
            Some(paintcraft_core::types::Rgba([17, 51, 93, 127])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        for format in [ImageExport::Tga, ImageExport::Webp] {
            for frontend in [
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                assert!(encode_quality_for_profile(&document, Some(format), 90, frontend).is_err());
            }
            let bytes =
                encode_quality_for_profile(&document, Some(format), 90, Frontend::Studio).unwrap();
            let decoded = decode_for_profile(
                &bytes,
                format!("picture.{}", format.extension()),
                Frontend::Studio,
            )
            .unwrap();
            assert_eq!(
                paintcraft_render::render_flat(&decoded.document).unwrap(),
                paintcraft_render::render_flat(&document).unwrap()
            );
        }
    }
    #[test]
    fn explicit_conversion_is_distinct_from_stock_open_and_does_not_bind_a_project() {
        // An exact tiny QOI RGB frame, independently written from the QOI wire layout.
        let mut bytes = b"qoif".to_vec();
        bytes.extend(1u32.to_be_bytes());
        bytes.extend(1u32.to_be_bytes());
        bytes.extend([4, 0, 0xff, 17, 51, 93, 127, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert!(decode_for_profile(&bytes, "sample.qoi".into(), Frontend::Studio).is_err());
        let loaded = decode_input(&bytes, "sample.qoi".into(), Frontend::Studio, true).unwrap();
        assert_eq!(loaded.document.canvas, (1, 1));
        let layer = &loaded.document.layers[&loaded.document.roots[0]];
        assert_eq!(layer.raster.get(0, 0).0, [17, 51, 93, 127]);
        assert!(decode_input(&bytes, "sample.qoi".into(), Frontend::ClassicV11, true).is_err());
    }
}

#[cfg(test)]
mod project_tests;

#[cfg(test)]
#[path = "files/specialist_exports.rs"]
mod specialist_exports;

#[cfg(test)]
#[path = "files/foreign_import.rs"]
mod foreign_import;
#[cfg(test)]
#[path = "files/psd_tests.rs"]
mod psd_tests;

pub(crate) fn pcx_mode(
    encoding: paintcraft_ui::pcx_export::Encoding,
) -> paintcraft_formats::pcx::Mode {
    use paintcraft_ui::pcx_export::Encoding;
    match encoding {
        Encoding::Rgb24 => paintcraft_formats::pcx::Mode::Rgb24,
        Encoding::IndexedExact => paintcraft_formats::pcx::Mode::IndexedExact,
        Encoding::Indexed256 => paintcraft_formats::pcx::Mode::Quantized256,
    }
}
#[cfg(test)]
fn encode_quality_for_profile(
    document: &Document,
    format: Option<ImageExport>,
    quality: u8,
    frontend: Frontend,
) -> Result<Vec<u8>> {
    encode_options_for_profile(document, format, quality, frontend, Default::default())
}
