//! Separate finite catalog export protocol7. Protocols4/5/6 remain unchanged.
use super::*;
mod request;
mod response;
mod worker;
pub use request::Header;
pub use response::{Frame, decode_response};
pub use worker::execute;
pub const PROTOCOL_VERSION: u16 = 7;
pub const REQUEST_HEADER: usize = 448;
pub const RESPONSE_HEADER: usize = 608;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;

pub const EXPORT_VARIANT: &str = "web-dds-texture-worker-v7";
pub const PROFILE: &str = "web-dds-pcnd32mib-4mp-v7";
