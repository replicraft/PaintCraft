//! Private native AV1 qualification boundary. No public AVIF read gate exists.
// Private qualification until the existing worker protocol gains the AVIF route.
#[cfg_attr(not(test), expect(dead_code))]
mod ffi;
mod heap;
