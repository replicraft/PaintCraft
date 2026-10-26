//! Explicit immutable float assessment, without document/save-binding publication.
use super::*;
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    selection: float_options::Selection,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
}
pub fn run(a: Options, json: bool) -> Reply {
    let result = float_job::prepare(
        &a.input,
        a.selection,
        a.expect_input_sha256,
        codec_worker::owner(),
        paintcraft_runtime::SharedBudget::new(512 << 20),
        std::sync::Arc::new(|| Ok(())),
        a.profile.frontend.into(),
    );
    match result {
        Ok(prepared) => {
            let result = prepared.inspection().map(|value| (value,
                "Float source assessed; import requires exact source/settings/assessment and loss consent".into()));
            // Retain the bounded receipt account through final JSON serialization.
            let reply = bare(json, result);
            drop(prepared);
            reply
        }
        Err(error) => bare(json, Err(error)),
    }
}
