use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
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
            let allowed = a.losses.allowed_for(&image::KNOWN_IMPORT_LOSSES)?;
            save::preflight(dest, a.output.overwrite)?;
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            let img = image::decode(&bytes)?;
            if let Some(l) = img.losses.iter().find(|l| !allowed.contains(l)) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    format!("import would lose {l}; acknowledge with --allow-loss {l}"),
                ));
            }
            let doc = import_image(img.width, img.height, &img.rgba, &sha)?;
            let ctx = Context {
                operation: "save-as",
                source_state: doc.state_id,
                source_sha256: Some(sha),
                losses_acknowledged: img.losses,
                destination: dest,
            };
            let (receipt, ok, summary) =
                write_output(dest, a.output.overwrite, pcnd::write(&doc), &ctx);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
