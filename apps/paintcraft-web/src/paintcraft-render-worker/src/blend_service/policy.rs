use paintcraft_core::types::{Error, ErrorCode};
pub(super) const JOB: u64 = 64 << 20;
pub(super) const PAYLOAD: u64 = 224 << 20;
pub(super) const BASE: u64 = 268435456;
pub(super) const NOISE: u64 = 34359738368;
pub(super) const CELLS: u64 = 4194304;
pub(super) const WINDOW: usize = 512;
pub(super) const FIXED: u64 = 8192;
pub(super) const ACK: u64 = 8192;
pub(super) const RETIRE: u64 = 8192;
pub(super) fn bad() -> Error {
    Error::new(ErrorCode::InputSchema, "Source8 service metadata")
}
pub(super) fn stale() -> Error {
    Error::new(ErrorCode::ConflictStaleBase, "Source8 service binding")
}
pub(super) fn bound() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source8 service allowance")
}
pub(super) fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Source8 standalone dispatch requires no ambient caller",
    )
}
