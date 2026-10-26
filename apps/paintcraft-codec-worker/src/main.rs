//! Single-request native decoder executable; never initializes GUI services.
#![deny(unsafe_code)]
#[cfg(target_arch = "wasm32")]
compile_error!("native codec helper is not a WebWorker implementation");
mod allocator;
mod worker;
#[global_allocator]
static HEAP: allocator::Limited<std::alloc::System, true> = allocator::Limited::new(
    std::alloc::System,
    paintcraft_formats::heif::wire::HEAP_BYTES as usize,
);
fn main() {
    if std::env::args_os().nth(1).is_some() || worker::run().is_err() {
        std::process::exit(2);
    }
}
