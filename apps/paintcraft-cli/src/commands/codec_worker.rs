//! Internal qualification consumer. Normal image Open/import stays registry-gated.
use super::*;
use paintcraft_formats::codec_worker as wire;
use paintcraft_platform::source::SourceIdentity;
use paintcraft_runtime::{Executor, Limits, process};
use std::time::Duration;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Codec {
    Heif,
    Jxl,
    Jxr,
}
impl Codec {
    fn wire(self) -> wire::Codec {
        match self {
            Self::Heif => wire::Codec::HeifGray8,
            Self::Jxl => wire::Codec::JxlRgba8,
            Self::Jxr => wire::Codec::JxrRgb8,
        }
    }
}
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[arg(long, value_enum)]
    codec: Codec,
    #[arg(long,default_value_t=wire::PIXELS_MAX,value_parser=clap::value_parser!(u64).range(1..=wire::PIXELS_MAX))]
    pixels: u64,
    #[arg(long,default_value_t=10000,value_parser=clap::value_parser!(u64).range(1..=120000))]
    deadline_ms: u64,
    #[arg(long,default_value_t=512<<20)]
    memory_limit: u64,
    #[arg(long)]
    expect_input_sha256: Option<String>,
}
impl Options {
    pub(super) fn jxl(input: PathBuf, expected: Option<String>) -> Self {
        Self {
            input,
            codec: Codec::Jxl,
            pixels: wire::PIXELS_MAX,
            deadline_ms: 10000,
            memory_limit: 512 << 20,
            expect_input_sha256: expected,
        }
    }
}
/// Conservative logical parent IO/verification slack; not a physical RSS cap.
const VERIFICATION_SLACK: u64 = 1 << 20;
pub(super) fn owner() -> wire::Owner {
    wire::Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    }
}
fn metadata(
    request: &wire::Request,
    frame: wire::Frame<'_>,
    helper_sha: [u8; 32],
) -> Result<Value> {
    let (model, pixels, alpha, encoding) = match frame.pixels {
        wire::Pixels::SrgbRgba8(rgba) => {
            ("srgb-rgba8", rgba, None, Value::str("srgb-straight-rgba8"))
        }
        wire::Pixels::NativeGray8 {
            gray,
            alpha,
            encoding,
        } => (
            "native-gray8",
            gray,
            alpha,
            Value::object([
                ("primaries", Value::Int(i64::from(encoding.primaries))),
                ("transfer", Value::Int(i64::from(encoding.transfer))),
                ("matrix", Value::Int(i64::from(encoding.matrix))),
                ("full_range", Value::Bool(encoding.full_range)),
            ]),
        ),
    };
    let source_jxr = if request.header.codec == wire::Codec::JxrRgb8 {
        Some(paintcraft_formats::jxr::inspect_checked(
            request.source.as_slice(),
            u64::from(request.header.pixels),
            &|| Ok(()),
        )?)
    } else {
        None
    };
    Ok(Value::object([
        ("schema", Value::str("paintcraft.codec-inspection/v1")),
        (
            "source_color_policy",
            source_jxr.map_or(Value::Null, |_| {
                Value::str(paintcraft_formats::jxr::COLOR_POLICY)
            }),
        ),
        (
            "source_resolution_f32_bits",
            source_jxr.map_or(Value::Null, |info| {
                Value::object([
                    (
                        "x",
                        info.resolution_x_bits
                            .map_or(Value::Null, |value| Value::str(format!("{value:08x}"))),
                    ),
                    (
                        "y",
                        info.resolution_y_bits
                            .map_or(Value::Null, |value| Value::str(format!("{value:08x}"))),
                    ),
                ])
            }),
        ),
        ("qualification", Value::str("internal-native-only")),
        ("reader_route", Value::str("internal-qualification")),
        ("width", Value::Int(i64::from(frame.width))),
        ("height", Value::Int(i64::from(frame.height))),
        ("model", Value::str(model)),
        ("encoding", encoding),
        ("samples_sha256", Value::str(hex(&Sha256::digest(pixels)))),
        (
            "alpha_sha256",
            alpha.map_or(Value::Null, |v| Value::str(hex(&Sha256::digest(v)))),
        ),
        ("source_sha256", Value::str(hex(&request.header.sha256))),
        ("helper_sha256", Value::str(hex(&helper_sha))),
    ]))
}
pub(super) fn with_frame<T>(
    options: Options,
    identity: SourceIdentity,
    length: u64,
    captured_owner: wire::Owner,
    cancel: paintcraft_runtime::Cancellation,
    current_owner: impl Fn() -> Result<wire::Owner>,
    consume: impl FnOnce(
        &wire::Request,
        wire::Frame<'_>,
        [u8; 32],
        &dyn Fn() -> Result<()>,
    ) -> Result<T>,
) -> Result<T> {
    wire::native::Job {
        input: options.input,
        identity,
        length,
        codec: options.codec.wire(),
        owner: captured_owner,
        pixels: options.pixels,
        deadline: Duration::from_millis(options.deadline_ms),
        application_version: env!("CARGO_PKG_VERSION").into(),
        expected_sha256: options.expect_input_sha256,
    }
    .consume(
        &|| cancel.check(),
        &current_owner,
        |launch, check| {
            process::run_parent_lease(
                launch.executable,
                launch.arguments,
                launch.request.segments(),
                process::Limits {
                    input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
                    output_bytes: wire::OUTPUT_BYTES_MAX,
                    deadline: launch.remaining,
                    resource_exit_code: Some(wire::RESOURCE_EXIT),
                },
                check,
            )
            .map(|output| output.bytes)
        },
        consume,
    )
}

fn inspect(
    options: Options,
    identity: SourceIdentity,
    length: u64,
    captured_owner: wire::Owner,
    cancel: paintcraft_runtime::Cancellation,
    current_owner: impl Fn() -> Result<wire::Owner>,
) -> Result<Value> {
    with_frame(
        options,
        identity,
        length,
        captured_owner,
        cancel,
        current_owner,
        |request, frame, helper_sha, _| metadata(request, frame, helper_sha),
    )
}
pub fn run(options: Options, json: bool) -> Reply {
    bare(
        json,
        (|| {
            let identity =
                SourceIdentity::existing_destination(&options.input)?.ok_or_else(|| {
                    Error::new(
                        ErrorCode::StorageFailed,
                        "Codec input must be a regular file",
                    )
                })?;
            let length = identity
                .file()
                .metadata()
                .map_err(|e| storage("codec source metadata", &e))?
                .len();
            let reservation = wire::reservation_bytes_for_input(length)?
                .checked_add(VERIFICATION_SLACK)
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "Codec reservation overflow")
                })?;
            let mut executor = Executor::new(Limits {
                workers: 1,
                pending_jobs: 1,
                transient_bytes: options.memory_limit,
            })?;
            let captured_owner = owner();
            let handle = executor.submit(
                reservation,
                move |cancel| {
                    inspect(options, identity, length, captured_owner, cancel, || {
                        Ok(captured_owner)
                    })
                },
                || {},
            )?;
            let value = loop {
                if let Some(value) = executor.poll_for(handle) {
                    break value?;
                }
                std::thread::sleep(Duration::from_millis(2));
            };
            Ok((value, "Internal native codec inspection completed".into()))
        })(),
    )
}
#[cfg(test)]
mod tests;
