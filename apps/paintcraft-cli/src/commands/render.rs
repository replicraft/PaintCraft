use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[arg(long, value_parser = ["png", "bmp", "jpeg", "jpg"])]
    format: Option<String>,
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
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
        _ => Err(Error::new(
            ErrorCode::InputSchema,
            "--format must be png, bmp or jpeg",
        )),
    });
    match by_name {
        Some(r) => r,
        None => ImageFormat::from_extension(dest).ok_or_else(|| {
            Error::new(
                ErrorCode::InputSchema,
                "cannot tell the image format; use a .png/.bmp/.jpg name or --format",
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
            let allowed = a.losses.allowed()?;
            let quality = a.quality;
            if !(1..=100).contains(&quality) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "--quality must be 1..=100",
                ));
            }
            save::preflight(dest, a.output.overwrite)?;
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            let doc = pcnd::read(&bytes)?;
            // Missing acknowledgements fail here, before anything is written.
            let flat = export::export_flat(&doc, format, quality, &allowed)?;
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
