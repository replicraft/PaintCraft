//! Bounded ordered encoders. Submitted work always completes before key publication.
use super::*;
use sources::Prepared;

pub(super) const SOURCE_BYTES: u64 = 1 << 20;
pub(super) const KEY_BYTES: usize = 1 << 20;
pub(super) const LAYERS: usize = 16;
pub(super) const ADMISSION_BYTES: u64 = 4 << 20;

pub(super) struct Batch<'a> {
    gpu: &'a GpuRenderer,
    encoder: Option<wgpu::CommandEncoder>,
    pending: Vec<Prepared>,
    source_bytes: u64,
    key_bytes: usize,
}
impl<'a> Batch<'a> {
    pub fn new(gpu: &'a GpuRenderer) -> Self {
        Self {
            gpu,
            encoder: None,
            pending: Vec::new(),
            source_bytes: 0,
            key_bytes: 0,
        }
    }
    pub fn before(
        &mut self,
        bytes: u64,
        cache: &mut SourceCache,
        check: &impl Fn() -> Result<()>,
        stats: &mut GpuRenderStats,
    ) -> Result<()> {
        if !self.pending.is_empty()
            && (!self.gpu.submission_batching
                || self.pending.len() == LAYERS
                || bytes > SOURCE_BYTES.saturating_sub(self.source_bytes)
                || self.key_bytes >= KEY_BYTES)
        {
            self.finish(cache, check, stats)?;
        }
        Ok(())
    }
    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder
            .get_or_insert_with(|| self.gpu.device.create_command_encoder(&Default::default()))
    }
    pub fn push(&mut self, source: Prepared) -> Result<()> {
        self.pending.try_reserve(1).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate GPU submission sources",
            )
        })?;
        self.source_bytes = self.source_bytes.saturating_add(source.buffer.size());
        self.key_bytes = self.key_bytes.saturating_add(source.metadata_bytes());
        self.pending.push(source);
        Ok(())
    }
    pub fn finish(
        &mut self,
        cache: &mut SourceCache,
        check: &impl Fn() -> Result<()>,
        stats: &mut GpuRenderStats,
    ) -> Result<()> {
        let Some(encoder) = self.encoder.take() else {
            return Ok(());
        };
        check()?;
        let submission = self.gpu.queue.submit([encoder.finish()]);
        stats.composition_submissions += 1;
        stats.max_submission_layers = stats.max_submission_layers.max(self.pending.len() as u32);
        stats.max_submission_source_bytes =
            stats.max_submission_source_bytes.max(self.source_bytes);
        stats.max_submission_metadata_bytes = stats
            .max_submission_metadata_bytes
            .max(self.key_bytes as u64);
        // Cancellation cannot abandon submitted resource ownership. Always observe
        // completion before checking it again, even when the consumer became stale.
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .map_err(|e| unavailable(format!("GPU layer batch failed: {e}")))?;
        check()?;
        for source in self.pending.drain(..) {
            cache.completed(source);
        }
        self.source_bytes = 0;
        self.key_bytes = 0;
        Ok(())
    }
}
