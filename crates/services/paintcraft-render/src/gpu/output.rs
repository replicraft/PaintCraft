//! One bounded completed compositor buffer. Display textures never enter this pool.
use super::*;
use std::ops::Deref;

// Native process evidence does not support globally enabling retention. Hosts
// may opt in explicitly; see docs/GPU_OUTPUT_POOL.md and its decision record.
pub const DEFAULT_OUTPUT_POOL_BYTES: u64 = 0;

#[derive(Debug)]
pub(super) struct Pool {
    pub limit: u64,
    retained: Option<wgpu::Buffer>,
}
impl Pool {
    pub fn new(limit: u64) -> Self {
        Self {
            limit,
            retained: None,
        }
    }
    pub fn clear(&mut self) {
        self.retained = None;
    }
    pub fn bytes(&self) -> u64 {
        self.retained.as_ref().map_or(0, wgpu::Buffer::size)
    }
}

/// Private lease: only explicit observed completion permits recycling. A cancelled,
/// failed or unwinding caller drops its buffer without reusing submitted work.
#[derive(Debug)]
pub(super) struct Output<'a> {
    pool: &'a Mutex<Pool>,
    buffer: Option<wgpu::Buffer>,
    pub reused: bool,
    completed: bool,
}
impl Deref for Output<'_> {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().expect("live compositor buffer")
    }
}
impl Output<'_> {
    pub fn completed(&mut self) {
        self.completed = true;
    }
}
impl Drop for Output<'_> {
    fn drop(&mut self) {
        if self.completed {
            let mut pool = self.pool.lock().unwrap_or_else(|p| p.into_inner());
            if self.size() <= pool.limit {
                pool.retained = self.buffer.take();
            }
        }
    }
}
impl GpuRenderer {
    pub(super) fn acquire_output(&self, size: u64) -> Output<'_> {
        let mut pool = self.output.lock().unwrap_or_else(|poisoned| {
            let mut pool = poisoned.into_inner();
            pool.clear();
            self.output.clear_poison();
            pool
        });
        let buffer = pool.retained.take().filter(|buffer| buffer.size() >= size);
        let copy_usage = if size <= pool.limit {
            wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::empty()
        };
        drop(pool);
        let reused = buffer.is_some();
        let buffer = buffer.unwrap_or_else(|| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("PaintCraft composite"),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | copy_usage,
                mapped_at_creation: false,
            })
        });
        Output {
            pool: &self.output,
            buffer: Some(buffer),
            reused,
            completed: false,
        }
    }
    pub fn output_pool_usage(&self) -> u64 {
        self.output
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .bytes()
    }
    /// Conservative retained-buffer allowance; no GPU mutex or physical-memory claim.
    pub fn output_pool_admission_bytes(&self) -> u64 {
        self.output_admission_bytes
    }
    /// Evict the idle buffer. Active renders are not cancelled and may warm the
    /// pool again after their completed work returns.
    pub fn clear_output_pool(&self) {
        self.output
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }
}
