use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    #[arg(long)]
    background: Option<String>,
    #[arg(long, default_value = POLICY)]
    policy: String,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// IDs are currently always deterministic.
    #[arg(long)]
    deterministic_ids: bool,
    #[command(flatten)]
    output: Output,
}

pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let (w, h) = (a.width, a.height);
            let background = a
                .background
                .as_deref()
                .map(|c| {
                    Rgba::parse(c).ok_or_else(|| {
                        Error::new(ErrorCode::InputSchema, "--background must be #RRGGBBAA")
                    })
                })
                .transpose()?;
            let dest = a.output.output.as_path();
            save::preflight(dest, a.output.overwrite)?;
            // Identifiers are always the deterministic allocator today; --deterministic-ids is accepted for forward compatibility.
            let doc = new_document(w, h, background, a.seed, &a.policy)?;
            let ctx = Context {
                operation: "save-as",
                source_state: doc.state_id,
                source_sha256: None,
                losses_acknowledged: vec![],
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
