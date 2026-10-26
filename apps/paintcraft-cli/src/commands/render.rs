use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    profile: super::format_profile::Options,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[arg(long, value_parser = ["png", "bmp", "jpeg", "jpg", "gif", "tiff", "tif", "tga", "webp", "jxl", "dds", "avif", "qoi", "pam", "ff", "pcx", "msp"])]
    format: Option<String>,
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
    /// PCX only: exact indexed colors or explicitly quantize to 256 colors.
    #[arg(long, value_enum)]
    pcx_encoding: Option<super::pcx_options::Encoding>,
    /// CPU render workers (one preserves the serial reference path).
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=64))]
    render_workers: u16,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}

fn output_format(format: Option<&str>, dest: &Path) -> Result<ImageFormat> {
    let by_name = format.map(|f| match f {
        "png" => Ok(ImageFormat::Png),
        "bmp" => Ok(ImageFormat::Bmp),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        "gif" => Ok(ImageFormat::Gif),
        "tiff" | "tif" => Ok(ImageFormat::Tiff),
        "tga" => Ok(ImageFormat::Tga),
        "webp" => Ok(ImageFormat::WebP),
        "jxl" => Ok(ImageFormat::Jxl),
        "dds" => Ok(ImageFormat::Dds),
        "avif" => Ok(ImageFormat::Avif),
        "qoi" => Ok(ImageFormat::Qoi),
        "pam" => Ok(ImageFormat::Pnm),
        "ff" => Ok(ImageFormat::Farbfeld),
        "pcx" => Ok(ImageFormat::Pcx),
        "msp" => Ok(ImageFormat::Msp),
        _ => Err(Error::new(
            ErrorCode::InputSchema,
            "--format must be png, bmp, jpeg, gif, tiff, tga, webp, jxl, dds, avif, qoi, pam, ff, pcx or msp",
        )),
    });
    match by_name {
        Some(r) => r,
        None => ImageFormat::from_extension(dest).ok_or_else(|| {
            Error::new(
                ErrorCode::InputSchema,
                "cannot tell the image format; use a supported filename extension or --format",
            )
        }),
    }
}

pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let input = &a.input;
            let dest = a.output.output.as_path();
            let format = output_format(a.format.as_deref(), dest)?;
            if a.pcx_encoding.is_some() && format != ImageFormat::Pcx {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "--pcx-encoding requires PCX output",
                ));
            }
            let family =
                paintcraft_formats::registry::Family::from_id(format.name()).ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Unregistered export format",
                    )
                })?;
            a.profile.admit_export(family, dest)?;
            let allowed = a.losses.allowed()?;
            let quality = a.quality;
            let mut options = export::FlatOptions::new(format, quality);
            options.pcx_mode = a.pcx_encoding.unwrap_or_default().into();
            if !(1..=100).contains(&quality) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "--quality must be 1..=100",
                ));
            }
            save::preflight(dest, a.output.overwrite)?;
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            if a.profile.admit_open(input, &bytes)? != paintcraft_formats::registry::Family::Pcnd {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Render expects a PCND project",
                ));
            }
            let doc = pcnd::read(&bytes)?;
            // Missing acknowledgements fail here, before anything is written.
            let flat = if a.render_workers == 1 {
                export::export_flat_options(&doc, options, &allowed)?
            } else {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(usize::from(a.render_workers))
                    .thread_name(|i| format!("paintcraft-export-{i}"))
                    .build()
                    .map_err(|e| {
                        Error::new(
                            ErrorCode::ResourceLimit,
                            format!("cannot create export workers: {e}"),
                        )
                    })?;
                export::export_flat_options_with(&doc, options, &allowed, |doc| {
                    paintcraft_render::parallel::render_in_pool(
                        &pool,
                        doc,
                        doc.canvas_rect(),
                        paintcraft_render::parallel::Partition::Bands { rows: 64 },
                        0,
                        || Ok(()),
                    )
                })?
            };
            let ctx = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: Some(sha),
                losses_acknowledged: flat.losses,
                destination: dest,
            };
            let (receipt, ok, summary) =
                write_output(dest, a.output.overwrite, Ok(flat.bytes), &ctx);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
