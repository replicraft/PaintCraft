//! Opaque immutable byte leases. Implementations own I/O; callers own interpretation.
use super::Result;
use std::{fmt::Debug, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpillUsage {
    /// Segment file lengths reserved against the quota, including dead extents.
    pub reserved_bytes: u64,
    pub segments: usize,
    pub cleanup_failures: u64,
}
/// Keeps its backing storage alive until the final reader releases the handle.
pub trait SpillBlob: Debug + Send + Sync {
    fn byte_len(&self) -> usize;
    /// Exactly `byte_len()` bytes; short reads, corruption and missing data fail.
    fn read_into(&self, destination: &mut [u8]) -> Result<()>;
}
pub trait SpillStore: Debug + Send + Sync {
    /// Prepare a complete immutable extent without invalidating old leases.
    /// Call `flush` successfully before publishing any returned handle.
    fn write(&self, bytes: &[u8]) -> Result<Arc<dyn SpillBlob>>;
    /// Complete the storage barrier for prepared extents, or fail before publication.
    fn flush(&self) -> Result<()>;
    /// Close the current append segment and reclaim segments without any leases.
    fn collect_garbage(&self) -> Result<SpillUsage>;
    fn usage(&self) -> SpillUsage;
}
