//! Single-request native decoder executable; never initializes GUI services.
#![deny(unsafe_code)]
#[cfg(target_arch = "wasm32")]
compile_error!("native codec helper is not a WebWorker implementation");
mod allocator;
mod worker;
#[global_allocator]
static HEAP: allocator::Limited<std::alloc::System, true> = allocator::Limited::new(
    std::alloc::System,
    paintcraft_formats::codec_worker::HEAP_BYTES as usize,
);
// Referenced read-only build record survives linker GC/stripping and can be
// checked without executing a foreign target. This is not a signature.
const _: () = assert!(paintcraft_formats::codec_worker::HEAP_BYTES == 268435456);
static BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V1\0",
    env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0END_CODEC_WORKER_BUILD\0",
)
.as_bytes();
fn main() {
    std::hint::black_box(BUILD_RECORD);
    if std::env::args_os().nth(1).is_some() || worker::run().is_err() {
        std::process::exit(2);
    }
}
