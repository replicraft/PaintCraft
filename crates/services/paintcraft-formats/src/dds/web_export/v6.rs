//! Separate finite catalog export protocol6. Protocol4/5 remain closed.
use super::*;
mod request;
mod response;
mod worker;
pub use request::Header;
pub use response::{Frame, decode_response};
pub use worker::execute;
pub const PROTOCOL_VERSION: u16 = 6;
pub const REQUEST_HEADER: usize = 384;
pub const RESPONSE_HEADER: usize = 544;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;

pub const EXPORT_VARIANT: &str = "web-dds-texture-worker-v6";
pub const PROFILE: &str = "web-dds-pcnd32mib-4mp-v6";
