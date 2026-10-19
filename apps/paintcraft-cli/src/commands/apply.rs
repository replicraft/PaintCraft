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
            let (bytes, sha) = read_bounded(input, INPUT_MAX, "input")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            let (ops, _) = read_bounded(&a.operations, OPERATIONS_MAX, "operations file")?;
            let mut session = Session::new(pcnd::read(&bytes)?);
            let receipt = session.apply_json(&ops)?;
            let ctx = Context {
                operation: if same_file(input.as_path(), dest) {
                    "save"
                } else {
                    "save-as"
                },
                source_state: session.document().state_id,
                source_sha256: Some(sha),
                losses_acknowledged: vec![],
                destination: dest,
            };
            let (file, ok, summary) =
                write_native(dest, a.output.overwrite, session.document(), &ctx);
            Ok(Written {
                engine: Some(receipt.to_value()),
                receipt: file,
                ok,
                summary,
            })
        })(),
    )
}
