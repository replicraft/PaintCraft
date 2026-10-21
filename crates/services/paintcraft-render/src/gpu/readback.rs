//! Complete mapping and fallible host ownership with unconditional unmapping.
use super::*;
struct Unmap<'a>(&'a wgpu::Buffer);
impl Drop for Unmap<'_> {
    fn drop(&mut self) {
        self.0.unmap();
    }
}
pub(super) fn copy_mapped(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
    check: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let _unmap = Unmap(buffer);
    let (send, recv) = mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .map_err(|e| unavailable(format!("GPU submission failed: {e}")))?;
    recv.recv_timeout(Duration::from_secs(30))
        .map_err(|e| unavailable(format!("GPU readback timed out: {e}")))?
        .map_err(|e| unavailable(format!("GPU readback failed: {e}")))?;
    check()?;
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| unavailable(format!("GPU mapping failed: {e}")))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(mapped.len()).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "cannot allocate GPU readback result",
        )
    })?;
    bytes.extend_from_slice(&mapped);
    drop(mapped);
    check()?;
    Ok(bytes)
}
