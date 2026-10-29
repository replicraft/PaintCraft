use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    operations: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    output: Output,
}

pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let input = &a.input;
            let dest = a.output.output.as_path();
            save::preflight(dest, a.output.overwrite)?;
            let in_place = same_file(input, dest);
            let (bytes, sha, identity) = read_regular_or_bounded(input, INPUT_MAX, "input")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            let (ops, _) = read_bounded(&a.operations, OPERATIONS_MAX, "operations file")?;
            let mut session = Session::new(super::source_metadata::read(&bytes)?);
            let receipt = session.apply_json(&ops)?;
            let ctx = Context {
                operation: if in_place { "save" } else { "save-as" },
                source_state: session.document().state_id,
                source_sha256: Some(sha),
                losses_acknowledged: vec![],
                destination: dest,
            };
            let (file, ok, summary) =
                write_native_guarded(dest, a.output.overwrite, session.document(), &ctx, || {
                    if let Some(identity) = &identity {
                        if !in_place && identity.matches_path(dest)? {
                            return Err(Error::new(
                                ErrorCode::InputSchema,
                                "copy output cannot replace the captured source project",
                            ));
                        }
                        identity.verify_bytes(
                            input,
                            bytes.len() as u64,
                            &Sha256::digest(&bytes).into(),
                            &|| Ok(()),
                        )?;
                    }
                    Ok(())
                });
            Ok(Written {
                engine: Some(receipt.to_value()),
                receipt: file,
                ok,
                summary,
            })
        })(),
    )
}
