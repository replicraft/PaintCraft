//! Weak derived snapshots: inactive editors never keep point buffers alive for a cache.
use super::*;
use paintcraft_tools::PreviewRevision;
use std::sync::{Mutex, Weak};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    tool_revision: PreviewRevision,
    queue_revision: PreviewRevision,
    document: (Uuid, u64),
    lineage: Uuid,
    tool: Tool,
    native: bool,
    pending: bool,
}
#[derive(Debug, Default)]
pub(super) struct Cache(Mutex<Option<(Stamp, Option<Weak<Preview>>)>>);

impl Editor {
    /// Reuse an immutable snapshot while a renderer owns it and its inputs match.
    /// An expired weak entry is rebuilt; the cache itself owns no point buffers.
    pub(crate) fn preview_snapshot(&self) -> Option<Arc<Preview>> {
        let stamp = Stamp {
            tool_revision: self.tools.preview_revision(),
            queue_revision: self.preview_queue_revision,
            document: self.render_key(),
            lineage: self.preview_lineage,
            tool: self.tools.tool,
            native: self.editing.is_some(),
            pending: self.pending_edit.is_some(),
        };
        if let Ok(cache) = self.preview_cache.0.lock()
            && let Some((cached_stamp, preview)) = cache.as_ref()
            && *cached_stamp == stamp
        {
            let weak = preview.as_ref()?;
            if let Some(preview) = weak.upgrade() {
                return Some(preview);
            }
        }
        let preview = self.collect_preview().map(Arc::new);
        if let Ok(mut cache) = self.preview_cache.0.lock() {
            *cache = Some((stamp, preview.as_ref().map(Arc::downgrade)));
        }
        preview
    }
    /// Owned compatibility view for callers that need to modify a preview.
    pub fn preview(&self) -> Option<Preview> {
        self.preview_snapshot().as_deref().cloned()
    }
}

#[cfg(test)]
#[path = "preview_cache/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "preview_cache/benchmark.rs"]
mod benchmark;
