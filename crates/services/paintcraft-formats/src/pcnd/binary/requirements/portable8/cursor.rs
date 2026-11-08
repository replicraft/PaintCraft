use super::*;
use wire::Cursor;
/// Borrowed framing only: ICC content/descriptor validation belongs to actual read.
#[derive(Clone, Copy)]
pub struct PortableHeaderView<'a> {
    pub document_id: uuid::Uuid,
    pub state_id: uuid::Uuid,
    pub canvas: (u32, u32),
    pub title: Option<&'a str>,
    pub source_metadata: Option<&'a [u8]>,
    pub layers: usize,
}
#[derive(Clone, Copy)]
pub struct PortableLayerHeader<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub visible: bool,
    pub opacity: u8,
    pub blend_mode: LayerBlendMode,
    pub origin: (i64, i64),
    pub width: u32,
    pub height: u32,
    pub default: Rgba,
    pub filters: usize,
}
#[derive(Clone, Copy)]
pub struct PortableMaskView<'a> {
    pub(super) domain: Rect,
    pub(super) enabled: bool,
    pub(super) bands: u64,
    pub(super) runs: u64,
    pub(super) bytes: &'a [u8],
}
impl PortableMaskView<'_> {
    pub fn domain(&self) -> Rect {
        self.domain
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn bands(&self) -> u64 {
        self.bands
    }
    pub fn runs(&self) -> u64 {
        self.runs
    }
    pub fn wire_bytes(&self) -> &[u8] {
        self.bytes
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PortableFilterView<'a> {
    pub id: uuid::Uuid,
    pub enabled: bool,
    pub descriptor: Descriptor,
    pub mask: Option<PortableMaskView<'a>>,
}
#[derive(Clone, Copy)]
pub struct PortableBlockView<'a> {
    pub x: u32,
    pub y: u32,
    pub encoding: u8,
    pub decoded_bytes: u32,
    stored: &'a [u8],
}
impl PortableBlockView<'_> {
    pub fn stored_bytes(&self) -> &[u8] {
        self.stored
    }
}
/// No raw domain object is borrowed or returned. Caller owns this inline handle.
pub struct PortableLayerCursor<'p, 'a> {
    plan: &'p PortableSource8Plan<'a>,
    index: usize,
}
impl<'a> PortableSource8Plan<'a> {
    pub fn header(&self) -> Result<PortableHeaderView<'_>> {
        let _control = self.helper_control()?;
        let mut c = Cursor::new(&self.checked.body()[self.header_start..], &self.meter);
        census::prefix(&mut c)
    }
    pub fn layer(&self, index: usize) -> Result<PortableLayerCursor<'_, 'a>> {
        let _control = self.helper_control()?;
        self.meter.step(16)?;
        if index >= self.layers.len() {
            return Err(invalid());
        }
        Ok(PortableLayerCursor { plan: self, index })
    }
    pub fn block(&self, layer: usize, index: usize) -> Result<PortableBlockView<'_>> {
        let _control = self.helper_control()?;
        self.meter.step(16)?;
        let layer = self.layers.get(layer).ok_or_else(invalid)?;
        if index >= layer.blocks_len as usize {
            return Err(invalid());
        }
        let b = self.blocks[layer.blocks_start as usize + index];
        let stored = &self.checked.body()
            [b.encoded_start as usize..b.encoded_start as usize + b.encoded_len as usize];
        Ok(PortableBlockView {
            x: b.x,
            y: b.y,
            encoding: b.encoding,
            decoded_bytes: b.decoded_bytes,
            stored,
        })
    }
}
impl PortableLayerCursor<'_, '_> {
    fn cursor(&self) -> Cursor<'_, '_> {
        let r = &self.plan.layers[self.index];
        Cursor::new(
            &self.plan.checked.body()[r.header_start as usize..r.header_end as usize],
            &self.plan.meter,
        )
    }
    pub fn header(&self) -> Result<PortableLayerHeader<'_>> {
        let _control = self.plan.helper_control()?;
        census::layer_header(&mut self.cursor())
    }
    pub fn filter(&self, index: usize) -> Result<PortableFilterView<'_>> {
        let _control = self.plan.helper_control()?;
        let mut c = self.cursor();
        let h = census::layer_header(&mut c)?;
        if index >= h.filters {
            return Err(invalid());
        }
        let mut masks = self.plan.limits.coverage;
        for i in 0..=index {
            let id = uuid::Uuid::from_bytes(c.id()?);
            let enabled = c.flag()?;
            let descriptor = c.descriptor_view(&self.plan.catalog)?;
            let mask = c.mask(&mut masks)?;
            if i == index {
                return Ok(PortableFilterView {
                    id,
                    enabled,
                    descriptor,
                    mask,
                });
            }
        }
        Err(invalid())
    }
    pub fn mask(&self) -> Result<Option<PortableMaskView<'_>>> {
        let _control = self.plan.helper_control()?;
        let mut c = self.cursor();
        let h = census::layer_header(&mut c)?;
        let mut masks = self.plan.limits.coverage;
        for _ in 0..h.filters {
            c.id()?;
            c.flag()?;
            c.descriptor_view(&self.plan.catalog)?;
            c.mask(&mut masks)?;
        }
        c.mask(&mut masks)
    }
    pub fn block_count(&self) -> usize {
        self.plan.layers[self.index].blocks_len as usize
    }
    pub fn block(&self, index: usize) -> Result<PortableBlockView<'_>> {
        self.plan.block(self.index, index)
    }
}

impl std::fmt::Debug for PortableLayerCursor<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableLayerCursor")
            .field("index", &self.index)
            .field("id", &self.plan.layers.get(self.index).map(|r| r.id))
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for PortableHeaderView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableHeaderView")
            .field("document_id", &self.document_id)
            .field("state_id", &self.state_id)
            .field("canvas", &self.canvas)
            .field("title_bytes", &self.title.map(str::len))
            .field(
                "source_metadata_bytes",
                &self.source_metadata.map(<[u8]>::len),
            )
            .field("layers", &self.layers)
            .finish()
    }
}
impl std::fmt::Debug for PortableLayerHeader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableLayerHeader")
            .field("id", &self.id)
            .field("name_bytes", &self.name.len())
            .field("visible", &self.visible)
            .field("opacity", &self.opacity)
            .field("blend_mode", &self.blend_mode)
            .field("origin", &self.origin)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("default", &self.default)
            .field("filters", &self.filters)
            .finish()
    }
}
impl std::fmt::Debug for PortableMaskView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableMaskView")
            .field("domain", &self.domain)
            .field("enabled", &self.enabled)
            .field("bands", &self.bands)
            .field("runs", &self.runs)
            .field("wire_bytes", &self.bytes.len())
            .finish()
    }
}
impl std::fmt::Debug for PortableBlockView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableBlockView")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("encoding", &self.encoding)
            .field("decoded_bytes", &self.decoded_bytes)
            .field("stored_bytes", &self.stored.len())
            .finish()
    }
}
