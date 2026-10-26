//! Native composition for exact Studio JXL/JXR/HEIF/AVIF import variants and private diagnostics.
use super::*;
use paintcraft_core::types::resources::ResourceLease;
use paintcraft_formats::codec_worker as wire;
use paintcraft_platform::source::SourceIdentity;
use paintcraft_runtime::{Executor, Limits};
use std::time::Duration;

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: super::format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
/// Extra logical declaration for the padded raster/index and streaming writer.
/// Fixed codec limits permit at most 256 full 256x256 RGBA tiles (64MiB), even
/// for thin images. This is not a global allocator, native-zstd or physical RSS cap.
const DOCUMENT_BYTES: u64 = 64 << 20;
const INDEX_AND_WRITER_BYTES: u64 = 17 << 20;
const IO_BYTES: u64 = 1 << 20;
struct Prepared {
    document: Document,
    identity: SourceIdentity,
    input: PathBuf,
    length: u64,
    sha: [u8; 32],
    owner: wire::Owner,
    cancel: paintcraft_runtime::Cancellation,
    assessment: Option<paintcraft_formats::jxr::ImportAssessment>,
    heif_assessment: Option<paintcraft_formats::heif::ImportAssessment>,
    avif_assessment: Option<paintcraft_formats::avif::ImportAssessment>,
    avif_sdr_assessment: Option<paintcraft_formats::avif::sdr::ImportAssessment>,
    // Last field: document and acquired resources drop before the runtime charge.
    _lease: ResourceLease,
}
impl Prepared {
    fn losses(&self) -> Vec<String> {
        let losses = self
            .assessment
            .as_ref()
            .map(|value| value.losses())
            .or_else(|| self.heif_assessment.as_ref().map(|value| value.losses()))
            .or_else(|| self.avif_assessment.as_ref().map(|value| value.losses()))
            .or_else(|| {
                self.avif_sdr_assessment
                    .as_ref()
                    .map(|value| value.losses())
            })
            .unwrap_or(&[]);
        losses.iter().map(|code| (*code).to_owned()).collect()
    }
    fn source_assessment(&self) -> Value {
        self.assessment
            .as_ref()
            .map(|value| value.to_value())
            .or_else(|| self.heif_assessment.as_ref().map(|value| value.to_value()))
            .or_else(|| self.avif_assessment.as_ref().map(|value| value.to_value()))
            .or_else(|| {
                self.avif_sdr_assessment
                    .as_ref()
                    .map(|value| value.to_value())
            })
            .unwrap_or(Value::Null)
    }
    fn check_owner(&self, owner: wire::Owner) -> Result<()> {
        self.cancel.check()?;
        if self.owner != owner {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Codec import owner changed",
            ));
        }
        Ok(())
    }
    fn check(&self, destination: &Path, owner: wire::Owner) -> Result<()> {
        self.check_owner(owner)?;
        if self.identity.matches_path(destination)? {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "conversion output must not replace its captured source",
            ));
        }
        self.identity
            .verify_bytes(&self.input, self.length, &self.sha, &|| {
                self.check_owner(owner)
            })
    }
    fn save(
        &self,
        output: &Output,
        current_owner: impl Fn() -> Result<wire::Owner>,
        reader_route: &str,
    ) -> Result<Written> {
        self.check(&output.output, current_owner()?)?;
        let check = || self.check_owner(current_owner()?);
        let outcome = save::save_with_preparation(
            &output.output,
            output.overwrite,
            save::STAGED_FILE_BYTES_MAX,
            |sink| pcnd::write_compact_to_checked(&self.document, sink, &check),
            |_| {
                self.check(&output.output, current_owner()?)?;
                self.check_owner(current_owner()?)
            },
        );
        let ctx = Context {
            operation: "save-as",
            source_state: self.document.state_id,
            source_sha256: Some(hex(&self.sha)),
            losses_acknowledged: self.losses(),
            destination: &output.output,
        };
        let (receipt, ok, summary) = finish_output(&output.output, &ctx, outcome);
        Ok(Written {
            engine: Some(Value::object([
                (
                    "decoder_profile",
                    Value::str(if self.heif_assessment.is_some() {
                        wire::NATIVE_HEIF_IMPORT_VARIANT
                    } else if self.assessment.is_some() {
                        wire::NATIVE_JXR_IMPORT_VARIANT
                    } else if self.avif_sdr_assessment.is_some() {
                        wire::NATIVE_AVIF_SDR_IMPORT_VARIANT
                    } else if self.avif_assessment.is_some() {
                        wire::NATIVE_AVIF_IMPORT_VARIANT
                    } else {
                        wire::NATIVE_JXL_IMPORT_VARIANT
                    }),
                ),
                ("conversion", Value::str("new-pcnd-copy")),
                ("reader_route", Value::str(reader_route)),
                (
                    "discarded_metadata",
                    Value::Bool(
                        self.assessment.is_some()
                            || self.heif_assessment.as_ref().is_some_and(|value| {
                                value.losses().contains(&"heif-item-names-not-retained")
                            })
                            || self.avif_sdr_assessment.as_ref().is_some_and(|value| {
                                value.losses().contains(&"avif-item-names-not-retained")
                            })
                            || self.avif_assessment.as_ref().is_some_and(|value| {
                                value.losses().contains(&"avif-item-names-not-retained")
                            }),
                    ),
                ),
                ("source_assessment", self.source_assessment()),
            ])),
            receipt,
            ok,
            summary,
        })
    }
}
fn reservation(length: u64) -> Result<u64> {
    wire::reservation_bytes_for_input(length)?
        .checked_add(DOCUMENT_BYTES + INDEX_AND_WRITER_BYTES + IO_BYTES)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Codec document reservation overflow",
            )
        })
}
fn prepare(
    job: wire::native::Job,
    allowed: Vec<String>,
    cancel: paintcraft_runtime::Cancellation,
    current_owner: impl Fn() -> Result<wire::Owner>,
    frontend: super::format_profile::Profile,
) -> Result<Prepared> {
    let length = job.length;
    let owner = job.owner;
    let lease = cancel.resource_lease().ok_or_else(|| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Codec import requires retained admission",
        )
    })?;
    let check = || cancel.check();
    let residency = paintcraft_runtime::SharedBudget::new(DOCUMENT_BYTES);
    let prepared = if job.codec == wire::Codec::AvifSrgb8 {
        let source = job.input.clone();
        job.prepare_avif_family(
            residency,
            lease.clone(),
            &check,
            &current_owner,
            |codec, _| {
                super::format_profile::Options { frontend }.admit_import_avif_codec(&source, codec)
            },
            super::codec_worker::launch,
        )
    } else {
        job.prepare_import(
            residency,
            lease.clone(),
            &check,
            &current_owner,
            super::codec_worker::launch,
        )
    }?;
    let adopted = prepared.adopt_with_losses(&allowed, &check, &current_owner)?;
    Ok(Prepared {
        document: adopted.document,
        identity: adopted.identity,
        input: adopted.input,
        length,
        sha: adopted.sha256,
        owner,
        cancel,
        assessment: adopted.jxr_assessment,
        heif_assessment: adopted.heif_assessment,
        avif_assessment: adopted.avif_assessment,
        avif_sdr_assessment: adopted.avif_sdr_assessment,
        _lease: lease,
    })
}

fn convert(
    input: &Path,
    output: &Output,
    expected: Option<String>,
    reader_route: &str,
    allowed: &[String],
    frontend: super::format_profile::Profile,
) -> Result<Written> {
    let codec = match input
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(paintcraft_formats::registry::Family::from_extension)
    {
        Some(paintcraft_formats::registry::Family::Jxl) => wire::Codec::JxlRgba8,
        Some(paintcraft_formats::registry::Family::Jxr) => wire::Codec::JxrRgb8,
        Some(paintcraft_formats::registry::Family::Heif) => wire::Codec::HeifSrgb8,
        Some(paintcraft_formats::registry::Family::Avif) => wire::Codec::AvifSrgb8,
        _ => {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Native codec import requires a qualified JXL/JXR/HEIF/AVIF filename",
            ));
        }
    };
    let allowed = allowed.to_vec();
    if same_file(input, &output.output) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "conversion output must not replace its source",
        ));
    }
    save::preflight(&output.output, output.overwrite)?;
    let identity = SourceIdentity::existing_destination(input)?.ok_or_else(|| {
        Error::new(
            ErrorCode::StorageFailed,
            "Codec input must be a regular file",
        )
    })?;
    let length = identity
        .file()
        .metadata()
        .map_err(|e| storage("codec input metadata", &e))?
        .len();
    let mut executor = Executor::new(Limits {
        workers: 1,
        pending_jobs: 1,
        transient_bytes: 512 << 20,
    })?;
    let owner = super::codec_worker::owner();
    let input = input.to_owned();
    let handle = executor.submit(
        reservation(length)?,
        move |cancel| {
            prepare(
                wire::native::Job {
                    input,
                    identity,
                    length,
                    codec,
                    owner,
                    pixels: wire::PIXELS_MAX,
                    deadline: Duration::from_secs(10),
                    application_version: env!("CARGO_PKG_VERSION").into(),
                    expected_sha256: expected,
                },
                allowed,
                cancel,
                || Ok(owner),
                frontend,
            )
        },
        || {},
    )?;
    let prepared = loop {
        if let Some(result) = executor.poll_for(handle) {
            break result?;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    // The prepared result owns the original runtime reservation through all
    // encoding, source/owner checks, staged publication, and document destruction.
    prepared.save(output, || Ok(owner), reader_route)
}
/// Normal import has no bypass: deny unavailable/profile-incompatible reads
/// before opening source bytes or starting a helper; only the exact native
/// Studio import variant is qualified.
pub(super) fn qualified(
    input: &Path,
    output: &Output,
    profile: &super::format_profile::Options,
    allowed: &[String],
) -> Result<Written> {
    profile.admit_import_native_path(input)?;
    convert(
        input,
        output,
        None,
        "native-studio-import",
        allowed,
        profile.frontend,
    )
}
/// Explicit hidden developer qualification, not a stock import/open capability.
pub fn run(options: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            if !matches!(
                options.profile.frontend,
                super::format_profile::Profile::Studio
            ) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Internal codec import qualification is Studio-only",
                ));
            }
            convert(
                &options.input,
                &options.output,
                options.expect_input_sha256,
                "internal-qualification",
                &options.losses.allowed_for(
                    &[
                        paintcraft_formats::jxr::KNOWN_IMPORT_LOSSES,
                        paintcraft_formats::heif::KNOWN_IMPORT_LOSSES,
                        paintcraft_formats::avif::KNOWN_IMPORT_LOSSES,
                        paintcraft_formats::avif::sdr::KNOWN_IMPORT_LOSSES,
                    ]
                    .concat(),
                )?,
                options.profile.frontend,
            )
        })(),
    )
}
#[cfg(test)]
mod tests;
