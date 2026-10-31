//! Separate spatial export protocol5. Protocol4 bytes/decoder remain closed.
use super::*;
mod request;
mod response;
mod worker;
pub use request::Header;
pub use response::{Frame, decode_response};
pub use worker::execute;
pub const PROTOCOL_VERSION: u16 = 5;
pub const REQUEST_HEADER: usize = 320;
pub const RESPONSE_HEADER: usize = 480;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;

pub const EXPORT_VARIANT: &str = "web-dds-texture-worker-v5";
pub const PROFILE: &str = "web-dds-pcnd32mib-4mp-v5";
