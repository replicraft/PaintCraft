use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    /// Explicit zero-based DCX/TIFF page; never selects a page implicitly.
    #[arg(long, conflicts_with = "entry")]
    page: Option<u16>,
    /// Explicit zero-based ICO/CUR/ICNS entry; never selects the largest image implicitly.
    #[arg(long)]
    entry: Option<u16>,
    #[command(flatten)]
    svg: super::svg_options::Arguments,
    #[command(flatten)]
    profile: super::format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}

pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let svg = a.svg.selection()?;
            let input = &a.input;
            if svg.is_some() {
                a.profile.admit_import_svg_path(input)?;
            } else if input
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                == Some(paintcraft_formats::registry::Family::Svg)
            {
                a.profile.admit_import_svg_path(input)?;
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "SVG import requires --svg-width and --svg-height",
                ));
            }
            let dest = a.output.output.as_path();
            if let Some(selection) = &svg {
                selection.protect_output(dest)?;
            }
            if same_file(input, dest) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "conversion output must not replace its source",
                ));
            }
            let known: Vec<&str> = image::KNOWN_IMPORT_LOSSES
                .iter()
                .chain(paintcraft_formats::project_import::KNOWN_LOSSES.iter())
                .chain(paintcraft_formats::icons::KNOWN_IMPORT_LOSSES.iter())
                .chain(paintcraft_formats::icns::KNOWN_IMPORT_LOSSES.iter())
                .chain(paintcraft_formats::jxr::KNOWN_IMPORT_LOSSES.iter())
                .chain(paintcraft_formats::heif::KNOWN_IMPORT_LOSSES.iter())
                .chain(paintcraft_formats::avif::KNOWN_IMPORT_LOSSES.iter())
                .chain(paintcraft_formats::avif::sdr::KNOWN_IMPORT_LOSSES.iter())
                .chain(paintcraft_formats::svg::KNOWN_LOSSES.iter())
                .copied()
                .chain(std::iter::once(
                    paintcraft_formats::tiff_pages::LOSS_OTHER_PAGES,
                ))
                .collect();
            let allowed = a.losses.allowed_for(&known)?;
            save::preflight(dest, a.output.overwrite)?;
            if svg.is_none()
                && input
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .and_then(paintcraft_formats::registry::Family::from_extension)
                    .is_some_and(|family| {
                        matches!(
                            family,
                            paintcraft_formats::registry::Family::Jxl
                                | paintcraft_formats::registry::Family::Jxr
                                | paintcraft_formats::registry::Family::Heif
                                | paintcraft_formats::registry::Family::Avif
                        )
                    })
            {
                if a.page.is_some() || a.entry.is_some() {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "Native codec import does not support page/entry selection",
                    ));
                }
                return super::codec_import::qualified(input, &a.output, &a.profile, &allowed);
            }
            let (bytes, sha, source_identity) = if let Some(limit) = input
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                .and_then(|family| match family {
                    paintcraft_formats::registry::Family::Msp => {
                        Some(paintcraft_formats::msp::BYTES_MAX)
                    }
                    paintcraft_formats::registry::Family::Icns => {
                        Some(paintcraft_formats::icns::BYTES_MAX)
                    }
                    paintcraft_formats::registry::Family::Dds => {
                        Some(paintcraft_formats::dds::BYTES_MAX)
                    }
                    paintcraft_formats::registry::Family::Tiff if a.page.is_some() => {
                        Some(paintcraft_formats::tiff_pages::BYTES_MAX)
                    }
                    _ => None,
                }) {
                let (bytes, sha, identity) = read_captured(input, limit as u64, "captured input")?;
                (bytes, sha, Some(identity))
            } else {
                read_conversion_source(
                    input,
                    if svg.is_some() {
                        paintcraft_formats::svg::SOURCE_MAX as u64
                    } else {
                        INPUT_MAX
                    },
                    "input",
                )?
            };
            let raw_sha256: [u8; 32] = Sha256::digest(&bytes).into();
            let family = if svg.is_some() {
                a.profile.admit_import_svg(input, &bytes)?
            } else {
                match (a.page, a.entry) {
                    (Some(page), _) => a.profile.admit_import_page(input, &bytes, page)?,
                    (_, Some(entry)) => a.profile.admit_import_entry(input, &bytes, entry)?,
                    _ => a.profile.admit_import(input, &bytes)?,
                }
            };
            if matches!(
                family,
                paintcraft_formats::registry::Family::Dds
                    | paintcraft_formats::registry::Family::Msp
            ) && source_identity.is_none()
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "DDS/MSP import requires a captured regular source",
                ));
            }
            if family == paintcraft_formats::registry::Family::Tiff
                && a.page.is_some()
                && source_identity.is_none()
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "TIFF page import requires a captured regular source",
                ));
            }
            let mut tiff_assessment = None;
            let (doc, losses) = if let Some(selection) = &svg {
                if !allowed.iter().any(|code| code == "svg-rasterized") {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "SVG rasterization requires --allow-loss svg-rasterized",
                    ));
                }
                selection.convert(&bytes, &sha)?
            } else if paintcraft_formats::project_import::supports(family) {
                let converted = if family == paintcraft_formats::registry::Family::Ora
                    && source_identity.is_some()
                {
                    a.profile.admit_import_ora(input, &bytes)?;
                    paintcraft_formats::project_import::decode_ora_with_neutral_groups(
                        &bytes,
                        4_194_304,
                        &|| Ok(()),
                    )?
                } else {
                    // Streams choose the existing strict flat profile before decoding, never fallback.
                    paintcraft_formats::project_import::decode_layered(
                        family,
                        &bytes,
                        4_194_304,
                        &|| Ok(()),
                    )?
                };
                (converted.document, converted.losses)
            } else if family == paintcraft_formats::registry::Family::Icns {
                let entry = a.entry.ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "ICNS requires an explicit entry")
                })?;
                let decoded = paintcraft_formats::icns::decode_entry(
                    &bytes,
                    entry,
                    image::DECODE_PIXELS_MAX,
                    &|| Ok(()),
                )?;
                (decoded.document(&sha)?, decoded.losses)
            } else if let Some(page) = a
                .page
                .filter(|_| family == paintcraft_formats::registry::Family::Tiff)
            {
                let converted = super::tiff_page::convert(&bytes, page, &sha, &|| Ok(()))?;
                tiff_assessment = Some(converted.assessment);
                (converted.document, converted.losses)
            } else if let Some(entry) = a.entry {
                let decoded = paintcraft_formats::icons::decode_entry(
                    &bytes,
                    entry,
                    image::DECODE_PIXELS_MAX,
                )?;
                (decoded.document(&sha)?, decoded.losses)
            } else {
                a.profile.reject_project_import(family)?;
                let img = if let Some(page) = a.page {
                    paintcraft_formats::dcx::decode_page(&bytes, page, image::DECODE_PIXELS_MAX)?
                } else {
                    image::decode(&bytes)?
                };
                let document_key = if let Some(page) = a.page {
                    paintcraft_document::RasterConversionKey::checked(
                        paintcraft_document::RasterConversionContext {
                            source_sha256: raw_sha256,
                            profile: paintcraft_formats::dcx::IMPORT_IDENTITY_PROFILE,
                            selection: paintcraft_document::RasterSelection::Page(page),
                            assessment_sha256: None,
                        },
                        img.width,
                        img.height,
                        &img.rgba,
                        image::DECODE_PIXELS_MAX,
                        &|| Ok(()),
                    )?
                    .hex()
                } else {
                    sha.clone()
                };
                (
                    import_image(img.width, img.height, &img.rgba, &document_key)?,
                    img.losses,
                )
            };
            if let Some(loss) = losses.iter().find(|loss| !allowed.contains(loss)) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    format!("import would lose {loss}; acknowledge with --allow-loss {loss}"),
                ));
            }
            if let Some(identity) = &source_identity {
                identity.verify_bytes(input, bytes.len() as u64, &raw_sha256, &|| Ok(()))?;
            }
            let ctx = Context {
                operation: "save-as",
                source_state: doc.state_id,
                source_sha256: Some(sha),
                losses_acknowledged: losses,
                destination: dest,
            };
            if source_identity
                .as_ref()
                .map(|identity| identity.matches_path(dest))
                .transpose()?
                .unwrap_or(false)
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "conversion output must not replace its captured imported source",
                ));
            }
            let (receipt, ok, summary) =
                write_native_guarded(dest, a.output.overwrite, &doc, &ctx, || {
                    if let Some(selection) = &svg {
                        selection.protect_output(dest)?;
                    }
                    if let Some(identity) = &source_identity {
                        if identity.matches_path(dest)? {
                            return Err(Error::new(
                                ErrorCode::InputSchema,
                                "conversion output must not replace its captured imported source",
                            ));
                        }
                        identity.verify_bytes(
                            input,
                            bytes.len() as u64,
                            &raw_sha256,
                            &|| Ok(()),
                        )?;
                    }
                    Ok(())
                });
            Ok(Written {
                engine: tiff_assessment.map(|assessment| {
                    Value::object([
                        (
                            "decoder_profile",
                            Value::str(paintcraft_formats::tiff_pages::IMPORT_VARIANT),
                        ),
                        ("source_assessment", assessment),
                    ])
                }),
                receipt,
                ok,
                summary,
            })
        })(),
    )
}

#[cfg(test)]
#[path = "import/source_tests.rs"]
mod source_tests;
