use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    /// Explicit zero-based page for a DCX import; never selects a page implicitly.
    #[arg(long)]
    page: Option<u16>,
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
            let input = &a.input;
            let dest = a.output.output.as_path();
            if same_file(input, dest) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "conversion output must not replace its source",
                ));
            }
            let known: Vec<&str> = image::KNOWN_IMPORT_LOSSES
                .iter()
                .chain(paintcraft_formats::project_import::KNOWN_LOSSES.iter())
                .copied()
                .collect();
            let allowed = a.losses.allowed_for(&known)?;
            save::preflight(dest, a.output.overwrite)?;
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            let family = match a.page {
                Some(page) => a.profile.admit_import_page(input, &bytes, page)?,
                None => a.profile.admit_import(input, &bytes)?,
            };
            let (doc, losses) = if matches!(
                family,
                paintcraft_formats::registry::Family::Paint
                    | paintcraft_formats::registry::Family::Pdn
            ) {
                let converted = paintcraft_formats::project_import::decode_layered(
                    family,
                    &bytes,
                    4_194_304,
                    &|| Ok(()),
                )?;
                (converted.document, converted.losses)
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
