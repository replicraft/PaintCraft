//! Headless stack flattening uses the shared recorded transaction and save guards.
use super::*;

#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    output: Output,
}

pub fn run(options: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = options.output.output.as_path();
            save::preflight(dest, options.output.overwrite)?;
            let in_place = same_file(&options.input, dest);
            let (bytes, sha, identity) =
                read_regular_or_bounded(&options.input, INPUT_MAX, "input")?;
            guard(options.expect_input_sha256.as_deref(), &sha)?;
            if format_profile::Options::default().admit_open(&options.input, &bytes)?
                != paintcraft_formats::registry::Family::Pcnd
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "flatten expects a PCND project",
                ));
            }
            let mut session = super::persistence::session(super::source_metadata::read(&bytes)?);
            let engine =
                session.flatten(|doc, check| paintcraft_render::render_cancellable(doc, check))?;
            let ctx = Context {
                operation: if in_place { "save" } else { "save-as" },
                source_state: session.document().state_id,
                source_sha256: Some(sha),
                losses_acknowledged: vec![],
                destination: dest,
            };
            let (receipt, ok, summary) = write_native_guarded(
                dest,
                options.output.overwrite,
                session.document(),
                &ctx,
                || {
                    if let Some(identity) = &identity {
                        if !in_place && identity.matches_path(dest)? {
                            return Err(Error::new(
                                ErrorCode::InputSchema,
                                "copy output cannot replace the captured source project",
                            ));
                        }
                        identity.verify_bytes(
                            &options.input,
                            bytes.len() as u64,
                            &Sha256::digest(&bytes).into(),
                            &|| Ok(()),
                        )?;
                    }
                    Ok(())
                },
            );
            Ok(Written {
                engine: Some(engine.to_value()),
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
