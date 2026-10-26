//! Actual native composition, kept internal until registry/runtime gates qualify.
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
    // Last field: document and acquired resources drop before the runtime charge.
    _lease: ResourceLease,
}
impl Prepared {
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
            losses_acknowledged: Vec::new(),
            destination: &output.output,
        };
        let (receipt, ok, summary) = finish_output(&output.output, &ctx, outcome);
        Ok(Written {
            engine: Some(Value::object([
                ("qualification", Value::str("internal-native-only")),
                ("conversion", Value::str("new-pcnd-copy")),
                ("public_reader_enabled", Value::Bool(false)),
                ("discarded_metadata", Value::Bool(false)),
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
    input: PathBuf,
    expected: Option<String>,
    identity: SourceIdentity,
    length: u64,
    owner: wire::Owner,
    cancel: paintcraft_runtime::Cancellation,
    current_owner: impl Fn() -> Result<wire::Owner>,
) -> Result<Prepared> {
    let lease = cancel.resource_lease().ok_or_else(|| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Codec import requires retained admission",
        )
    })?;
    let captured_cancel = cancel.clone();
    let captured_identity = identity.clone();
    let captured_input = input.clone();
    super::codec_worker::with_frame(
        super::codec_worker::Options::jxl(input, expected),
        identity,
        length,
        owner,
        cancel,
        current_owner,
        |request, frame, _, check| {
            let wire::Pixels::SrgbRgba8(rgba) = frame.pixels else {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Only qualified straight sRGB RGBA8 can become a document",
                ));
            };
            let document = paintcraft_document::import_image_checked(
                frame.width,
                frame.height,
                rgba,
                &hex(&request.header.sha256),
                check,
            )?;
            check()?;
            Ok(Prepared {
                document,
                identity: captured_identity,
                input: captured_input,
                length,
                sha: request.header.sha256,
                owner,
                cancel: captured_cancel,
                _lease: lease,
            })
        },
    )
}
fn convert(input: &Path, output: &Output, expected: Option<String>) -> Result<Written> {
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
            prepare(input, expected, identity, length, owner, cancel, || {
                Ok(owner)
            })
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
    prepared.save(output, || Ok(owner))
}
/// Normal import has no bypass: deny unavailable/profile-incompatible reads
/// before opening source bytes or starting a helper. Registry remains unchanged.
pub(super) fn qualified(
    input: &Path,
    output: &Output,
    profile: &super::format_profile::Options,
) -> Result<Written> {
    profile.admit_import_jxl_path(input)?;
    convert(input, output, None)
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
            convert(&options.input, &options.output, options.expect_input_sha256)
        })(),
    )
}
#[cfg(test)]
mod tests;
