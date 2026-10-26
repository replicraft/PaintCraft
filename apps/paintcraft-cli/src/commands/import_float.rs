//! Digest-bound conversion into a fresh PCND copy, using the existing staged saver.
use super::*;
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    selection: float_options::Selection,
    #[arg(long)]
    expect_input_sha256: String,
    #[arg(long)]
    expect_assessment_sha256: String,
    #[command(flatten)]
    profile: format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
pub fn run(a: Options, json: bool) -> Reply {
    let result: Result<(float_job::Adopted, Written)> = (|| {
        let expected = float_job::digest(&a.expect_assessment_sha256)?;
        float_job::digest(&a.expect_input_sha256)?;
        let allowed = a.losses.allowed_for(
            &[
                paintcraft_formats::float_import::KNOWN_LOSSES,
                paintcraft_formats::precision::KNOWN_LOSSES,
            ]
            .concat(),
        )?;
        if a.output
            .output
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("pcnd"))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Float import destination must be a fresh .pcnd project",
            ));
        }
        let (_, codec, _) = float_job::profile(&a.input)?;
        float_job::admit(&a.input, a.profile.frontend.into(), codec)?;
        save::preflight(&a.output.output, a.output.overwrite)?;
        let prepared = float_job::prepare(
            &a.input,
            a.selection,
            Some(a.expect_input_sha256),
            codec_worker::owner(),
            paintcraft_runtime::SharedBudget::new(512 << 20),
            std::sync::Arc::new(|| Ok(())),
            a.profile.frontend.into(),
        )?;
        let inspection = prepared.inspection()?;
        let adopted = prepared.adopt(expected, &allowed)?;
        adopted.protect(&a.output.output)?;
        let assessment = adopted
            .value
            .float_assessment
            .as_ref()
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InvariantViolation,
                    "Float adoption lost assessment",
                )
            })?
            .assessment();
        let context = Context {
            operation: "save-as",
            source_state: adopted.value.document.state_id,
            source_sha256: Some(hex(&adopted.value.sha256)),
            losses_acknowledged: assessment
                .losses()
                .iter()
                .map(|v| (*v).to_owned())
                .collect(),
            destination: &a.output.output,
        };
        let outcome = save::save_with_preparation(
            &a.output.output,
            a.output.overwrite,
            save::STAGED_FILE_BYTES_MAX,
            |sink| {
                pcnd::write_compact_to_checked(&adopted.value.document, sink, &|| adopted.check())
            },
            |_| adopted.protect(&a.output.output),
        );
        let (receipt, ok, summary) = finish_output(&a.output.output, &context, outcome);
        Ok((
            adopted,
            Written {
                engine: Some(inspection),
                receipt,
                ok,
                summary,
            },
        ))
    })();
    match result {
        Ok((adopted, written)) => {
            let reply = finish(json, Ok(written));
            drop(adopted);
            reply
        }
        Err(error) => finish(json, Err(error)),
    }
}
