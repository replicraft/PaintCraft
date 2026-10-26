//! Single-request native decoder executable; never initializes GUI services.
#![deny(unsafe_code)]
#[cfg(target_arch = "wasm32")]
compile_error!("native codec helper is not a WebWorker implementation");
mod allocator;
#[cfg(feature = "avif-codec")]
mod avif;
mod jxr;
#[cfg(feature = "jxr-encode")]
mod jxr_encode;
mod parent_lease;
mod worker;
#[global_allocator]
static HEAP: allocator::Limited<std::alloc::System, true> = allocator::Limited::new(
    std::alloc::System,
    paintcraft_formats::codec_worker::HEAP_BYTES as usize,
);
// Referenced read-only build record survives linker GC/stripping and can be
// checked without executing a foreign target. This is not a signature.
const _: () = assert!(paintcraft_formats::codec_worker::HEAP_BYTES == 268435456);
#[cfg(not(feature = "avif-codec"))]
static BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V4\0",
    env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0END_CODEC_WORKER_BUILD\0",
)
.as_bytes();
#[cfg(all(feature = "avif-codec", not(feature = "avif-sdr-codec")))]
static BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V5_AVIF\0",
    env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0END_CODEC_WORKER_BUILD\0",
).as_bytes();
#[cfg(all(feature = "avif-sdr-codec", not(feature = "float-codecs")))]
static BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V6_AVIF_SDR\0", env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0END_CODEC_WORKER_BUILD\0",
).as_bytes();
#[cfg(all(feature = "float-codecs", not(feature = "jxr-encode")))]
static BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V7_FLOAT\0",
    env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0",
    "AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0",
    "FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576\0END_CODEC_WORKER_BUILD\0",
)
.as_bytes();
#[cfg(feature = "jxr-encode")]
static BUILD_RECORD: &[u8] = paintcraft_formats::codec_worker::jxr_encode::candidate::BUILD_RECORD;
fn main() {
    std::hint::black_box(BUILD_RECORD);
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        #[cfg(feature = "jxr-encode")]
        [flag] if flag == "--jxr-encode" => jxr_encode::run(),
        #[cfg(feature = "jxr-encode")]
        [flag, path] if flag == "--jxr-writer-source" => {
            worker::run_writer(std::path::Path::new(path))
        }
        [] => worker::run(None),
        [flag, path] if flag == "--jxr-source" => worker::run(Some(std::path::Path::new(path))),
        _ => std::process::exit(2),
    };
    if result.is_err() {
        std::process::exit(2);
    }
}
