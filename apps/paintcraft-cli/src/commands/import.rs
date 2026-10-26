use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    /// Explicit zero-based page for a DCX import; never selects a page implicitly.
    #[arg(long, conflicts_with = "entry")]
    page: Option<u16>,
    /// Explicit zero-based ICO/CUR entry; never selects the largest image implicitly.
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
                .chain(paintcraft_formats::svg::KNOWN_LOSSES.iter())
                .copied()
                .collect();
            let allowed = a.losses.allowed_for(&known)?;
            save::preflight(dest, a.output.overwrite)?;
            if svg.is_none()
                && input
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .and_then(paintcraft_formats::registry::Family::from_extension)
                    == Some(paintcraft_formats::registry::Family::Jxl)
            {
                if a.page.is_some() || a.entry.is_some() {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "JXL does not support page/entry selection",
                    ));
                }
                return super::codec_import::qualified(input, &a.output, &a.profile);
            }
            let (bytes, sha, source_identity) = if input
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                == Some(paintcraft_formats::registry::Family::Msp)
            {
                let (bytes, sha, identity) = read_captured(
                    input,
                    paintcraft_formats::msp::BYTES_MAX as u64,
                    "MSP input",
                )?;
                (bytes, sha, Some(identity))
            } else {
                let (bytes, sha) = read_bounded(
                    input,
                    if svg.is_some() {
                        paintcraft_formats::svg::SOURCE_MAX as u64
                    } else {
                        INPUT_MAX
                    },
                    "input",
                )?;
                (bytes, sha, None)
            };
            let family = if svg.is_some() {
                a.profile.admit_import_svg(input, &bytes)?
            } else {
                match (a.page, a.entry) {
                    (Some(page), _) => a.profile.admit_import_page(input, &bytes, page)?,
                    (_, Some(entry)) => a.profile.admit_import_entry(input, &bytes, entry)?,
                    _ => a.profile.admit_import(input, &bytes)?,
                }
            };
            let (doc, losses) = if let Some(selection) = svg {
                if !allowed.iter().any(|code| code == "svg-rasterized") {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "SVG rasterization requires --allow-loss svg-rasterized",
                    ));
                }
                selection.convert(&bytes, &sha)?
            } else if paintcraft_formats::project_import::supports(family) {
                let converted = paintcraft_formats::project_import::decode_layered(
                    family,
                    &bytes,
                    4_194_304,
                    &|| Ok(()),
                )?;
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
                (
                    import_image(img.width, img.height, &img.rgba, &sha)?,
                    img.losses,
                )
            };
            if let Some(loss) = losses.iter().find(|loss| !allowed.contains(loss)) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    format!("import would lose {loss}; acknowledge with --allow-loss {loss}"),
                ));
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
                    "conversion output must not replace its captured MSP source",
                ));
            }
            let (receipt, ok, summary) = write_native(dest, a.output.overwrite, &doc, &ctx);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
