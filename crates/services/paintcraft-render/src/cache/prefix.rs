use super::*;
use crate::compose_roots_into;

impl CompositeCache {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_miss(
        &mut self,
        doc: &Document,
        rect: Rect,
        domain: Domain,
        key_budget: usize,
        cacheable: bool,
        check: &impl Fn() -> Result<()>,
        stats: &mut RenderStats,
    ) -> Result<Vec<u8>> {
        let relevant = |layer: &&paintcraft_document::Layer| {
            layer.visible && layer.opacity != 0 && rect.intersect(&layer.extent()).is_some()
        };
        let mut indices = doc
            .layers_in_order()
            .enumerate()
            .filter_map(|(i, layer)| relevant(&layer).then_some(i));
        let Some(first) = indices.next() else {
            return render_region_cancellable(doc, rect, check);
        };
        let end = indices.last().unwrap_or(first);
        // A one-layer tile has no reusable lower-layer prefix.
        if !cacheable || end == first {
            return render_region_cancellable(doc, rect, check);
        }
        let Some(key) = Key::build_prefix(doc, rect, key_budget, end)? else {
            return render_region_cancellable(doc, rect, check);
        };
        let mut pixels = output_buffer(region_bytes(doc, rect)?)?;
        let hit = if domain == Domain::Preview {
            self.committed.find(&key).map(|i| (Domain::Committed, i))
        } else {
            None
        }
        .or_else(|| {
            match domain {
                Domain::Committed => self.committed.find(&key),
                Domain::Preview => self.preview.find(&key),
            }
            .map(|i| (domain, i))
        });
        if let Some((domain, index)) = hit {
            let partition = match domain {
                Domain::Committed => &mut self.committed,
                Domain::Preview => &mut self.preview,
            };
            pixels.copy_from_slice(partition.touch(index));
            stats.prefix_hits += 1;
        } else {
            compose_roots_into(doc, rect, 0..end, &mut pixels, check)?;
            check()?;
            // The returned working tile and cached prefix have separate ownership.
            let mut saved = output_buffer(pixels.len())?;
            saved.copy_from_slice(&pixels);
            let partition = match domain {
                Domain::Committed => &mut self.committed,
                Domain::Preview => &mut self.preview,
            };
            partition.insert(key, saved, true);
        }
        compose_roots_into(doc, rect, end..doc.roots.len(), &mut pixels, check)?;
        Ok(pixels)
    }
}
