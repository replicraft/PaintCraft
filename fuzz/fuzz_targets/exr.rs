#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|bytes: &[u8]| {
    let _ = paintcraft_container_fuzz::classify(paintcraft_container_fuzz::exercise(
        paintcraft_container_fuzz::Target::Exr,
        bytes,
    ));
});
