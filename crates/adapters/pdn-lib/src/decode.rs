//! A narrow typed PDN3 bitmap profile. Unknown objects never become active types.
use super::*;
use nrbf::{Graph, Value, boolean, integer};
mod chunks;

#[derive(Clone, Copy, Debug)]
pub struct DecodeLimits {
    pub envelope: Limits,
    pub graph_bytes: usize,
    pub graph_metadata_bytes: usize,
    pub objects: usize,
    pub array_items: usize,
    pub members: usize,
    pub object_depth: usize,
    pub decoded_pixels: u64,
    pub decoded_bytes: usize,
    pub chunks: usize,
}
impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            envelope: Limits::default(),
            graph_bytes: 16 << 20,
            graph_metadata_bytes: 32 << 20,
            objects: 4096,
            array_items: 16384,
            members: 64,
            object_depth: 64,
            decoded_pixels: 268_435_456,
            decoded_bytes: 1 << 30,
            chunks: 65536,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    MetadataUnretained,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedLayer<'a> {
    pub object_id: i32,
    pub name: &'a str,
    pub visible: bool,
    pub opacity: u8,
    pub is_background: bool,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded<'a> {
    pub width: u32,
    pub height: u32,
    pub saved_with: [u32; 4],
    /// Independent untrusted XML claim; some writers leave saved_with at zero.
    pub header_version: Option<String>,
    pub layers: Vec<DecodedLayer<'a>>,
    /// EXIF/custom metadata and source-specific background-role semantics are
    /// not represented by the RGBA8 conversion DTO.
    pub diagnostics: [Diagnostic; 1],
}
struct Plan<'a> {
    object_id: i32,
    memory: i32,
    name: &'a str,
    visible: bool,
    opacity: u8,
    is_background: bool,
    length: usize,
}
fn unsigned(value: Value) -> Result<u32> {
    u32::try_from(integer(value)?).map_err(|_| malformed("PDN integer is outside unsigned bounds"))
}
fn vec_checked<T>(n: usize, limit: usize) -> Result<Vec<T>> {
    if n.checked_mul(std::mem::size_of::<T>())
        .is_none_or(|bytes| bytes > limit)
    {
        return Err(resource("PDN conversion metadata exceeds limits"));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(n)
        .map_err(|_| resource("Cannot allocate PDN conversion metadata"))?;
    Ok(values)
}
fn metadata(graph: &Graph<'_>, value: Value, check: &dyn Fn() -> Result<()>) -> Result<()> {
    if value == Value::Null {
        return Ok(());
    }
    for value in graph.array(value)? {
        check()?;
        let pair = graph.object(*value, nrbf::PAIRS)?;
        graph.exact_fields(pair, &["key", "value"])?;
        for key in ["key", "value"] {
            let value = graph.field(pair, key)?;
            if value != Value::Null {
                graph.text(value)?;
            }
        }
    }
    Ok(())
}
fn layer_plan<'a>(
    graph: &Graph<'a>,
    value: Value,
    size: (u32, u32),
    check: &dyn Fn() -> Result<()>,
) -> Result<Plan<'a>> {
    let layer = graph.object(value, "PaintDotNet.BitmapLayer")?;
    graph.exact_fields(
        layer,
        &[
            "properties",
            "surface",
            "Layer+isDisposed",
            "Layer+width",
            "Layer+height",
            "Layer+properties",
        ],
    )?;
    if boolean(graph.field(layer, "Layer+isDisposed")?)?
        || (
            unsigned(graph.field(layer, "Layer+width")?)?,
            unsigned(graph.field(layer, "Layer+height")?)?,
        ) != size
    {
        return Err(unsupported(
            "PDN disposed or differently sized bitmap layers are unqualified",
        ));
    }
    let bitmap = graph.object(
        graph.field(layer, "properties")?,
        "PaintDotNet.BitmapLayer+BitmapLayerProperties",
    )?;
    graph.exact_fields(bitmap, &["blendOp"])?;
    let blend = graph.object(
        graph.field(bitmap, "blendOp")?,
        "PaintDotNet.UserBlendOps+NormalBlendOp",
    )?;
    graph.exact_fields(blend, &[])?;
    let properties = graph.object(
        graph.field(layer, "Layer+properties")?,
        "PaintDotNet.Layer+LayerProperties",
    )?;
    graph.exact_fields(
        properties,
        &[
            "name",
            "userMetadataItems",
            "visible",
            "isBackground",
            "opacity",
            "blendMode",
        ],
    )?;
    let mode = graph.object(
        graph.field(properties, "blendMode")?,
        "PaintDotNet.LayerBlendMode",
    )?;
    graph.exact_fields(mode, &["value__"])?;
    if integer(graph.field(mode, "value__")?)? != 0 {
        return Err(unsupported(
            "PDN non-Normal blend modes require a qualified compositor",
        ));
    }
    metadata(graph, graph.field(properties, "userMetadataItems")?, check)?;
    let surface = graph.object(graph.field(layer, "surface")?, "PaintDotNet.Surface")?;
    graph.exact_fields(surface, &["width", "height", "stride", "scan0"])?;
    if (
        unsigned(graph.field(surface, "width")?)?,
        unsigned(graph.field(surface, "height")?)?,
    ) != size
        || u64::from(unsigned(graph.field(surface, "stride")?)?) != u64::from(size.0) * 4
    {
        return Err(unsupported(
            "PDN surface dimensions, strides or padding are unqualified",
        ));
    }
    let memory = graph.object(graph.field(surface, "scan0")?, "PaintDotNet.MemoryBlock")?;
    graph.exact_fields(memory, &["length64", "hasParent", "deferred"])?;
    if boolean(graph.field(memory, "hasParent")?)? || !boolean(graph.field(memory, "deferred")?)? {
        return Err(unsupported(
            "PDN requires independent deferred pixel buffers",
        ));
    }
    let length = usize::try_from(integer(graph.field(memory, "length64")?)?)
        .map_err(|_| resource("PDN buffer length exceeds target address space"))?;
    if length as u128 != u128::from(size.0) * u128::from(size.1) * 4 {
        return Err(malformed("PDN buffer size disagrees with the canvas"));
    }
    let opacity = u8::try_from(unsigned(graph.field(properties, "opacity")?)?)
        .map_err(|_| malformed("PDN opacity exceeds 8 bits"))?;
    Ok(Plan {
        object_id: layer.id,
        memory: memory.id,
        name: graph.text(graph.field(properties, "name")?)?,
        visible: boolean(graph.field(properties, "visible")?)?,
        opacity,
        is_background: boolean(graph.field(properties, "isBackground")?)?,
        length,
    })
}
pub fn decode_layers<'a>(
    bytes: &'a [u8],
    limits: &DecodeLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded<'a>> {
    let inspected = inspect(bytes, &limits.envelope, check)?;
    if !matches!(inspected.body_header, BodyHeader::DeferredNrbf(_)) {
        return Err(unsupported(
            "Only deferred PDN3 typed raster documents are decoded",
        ));
    }
    let graph = Graph::parse(
        &inspected.body[2..],
        nrbf::GraphLimits {
            bytes: limits.graph_bytes,
            metadata: limits.graph_metadata_bytes,
            objects: limits.objects,
            array_items: limits.array_items,
            members: limits.members,
            depth: limits.object_depth,
        },
        check,
    )?;
    graph.validate_raster_schema(check)?;
    let root = graph.object(Value::Ref(graph.root), "PaintDotNet.Document")?;
    graph.exact_fields(
        root,
        &[
            "isDisposed",
            "layers",
            "width",
            "height",
            "savedWith",
            "userMetadataItems",
        ],
    )?;
    if boolean(graph.field(root, "isDisposed")?)? {
        return Err(unsupported("Disposed PDN documents are unsupported"));
    }
    let size = (
        unsigned(graph.field(root, "width")?)?,
        unsigned(graph.field(root, "height")?)?,
    );
    if size != (inspected.hints.width, inspected.hints.height) {
        return Err(malformed("PDN XML and graph dimensions disagree"));
    }
    metadata(&graph, graph.field(root, "userMetadataItems")?, check)?;
    let version = graph.object(graph.field(root, "savedWith")?, "System.Version")?;
    graph.exact_fields(version, &["_Major", "_Minor", "_Build", "_Revision"])?;
    let saved_with = [
        unsigned(graph.field(version, "_Major")?)?,
        unsigned(graph.field(version, "_Minor")?)?,
        unsigned(graph.field(version, "_Build")?)?,
        unsigned(graph.field(version, "_Revision")?)?,
    ];
    let list = graph.object(graph.field(root, "layers")?, "PaintDotNet.LayerList")?;
    graph.exact_fields(
        list,
        &[
            "parent",
            "ArrayList+_items",
            "ArrayList+_size",
            "ArrayList+_version",
        ],
    )?;
    if graph.field(list, "parent")? != Value::Ref(root.id) {
        return Err(malformed("PDN layer list belongs to another document"));
    }
    integer(graph.field(list, "ArrayList+_version")?)?;
    let count = unsigned(graph.field(list, "ArrayList+_size")?)? as usize;
    if count == 0
        || count > limits.envelope.layers as usize
        || count != inspected.hints.layers as usize
    {
        return Err(malformed("PDN XML and graph layer counts disagree"));
    }
    let values = graph.array(graph.field(list, "ArrayList+_items")?)?;
    if values.len() < count || values[count..].iter().any(|v| *v != Value::Null) {
        return Err(malformed("PDN layer-list size disagrees with its array"));
    }
    let total = u64::from(size.0) * u64::from(size.1);
    let total = total
        .checked_mul(count as u64)
        .ok_or_else(|| resource("PDN aggregate pixel count overflow"))?;
    if total > limits.decoded_pixels
        || total
            .checked_mul(4)
            .is_none_or(|bytes| bytes > limits.decoded_bytes as u64)
    {
        return Err(resource("PDN aggregate decoded pixel budget exceeded"));
    }
    let mut plans = vec_checked(count, limits.graph_metadata_bytes)?;
    for value in &values[..count] {
        check()?;
        let plan = layer_plan(&graph, *value, size, check)?;
        if plans
            .iter()
            .any(|old: &Plan<'_>| old.memory == plan.memory || old.object_id == plan.object_id)
        {
            return Err(unsupported(
                "Shared PDN layer objects or pixel buffers are unqualified",
            ));
        }
        plans.push(plan);
    }
    let mut order = vec_checked(count, limits.graph_metadata_bytes)?;
    for node in &graph.nodes {
        if graph.class(node) == Some("PaintDotNet.MemoryBlock") {
            order.push(node);
        }
    }
    if order.len() != plans.len() {
        return Err(unsupported(
            "PDN contains unreferenced or additional pixel buffers",
        ));
    }
    order.sort_unstable_by_key(|node| node.offset);
    let mut decoded = vec_checked(count, limits.graph_metadata_bytes)?;
    let mut offset = inspected.body_offset + 2 + graph.consumed;
    for memory in order {
        check()?;
        let index = plans
            .iter()
            .position(|plan| plan.memory == memory.id)
            .ok_or_else(|| malformed("PDN deferred buffer has no layer"))?;
        let plan = &plans[index];
        let (rgba, consumed) = chunks::decode(&bytes[offset..], plan.length, limits, check)?;
        offset = offset
            .checked_add(consumed)
            .ok_or_else(|| resource("PDN pixel stream offset overflow"))?;
        decoded.push((
            index,
            DecodedLayer {
                object_id: plan.object_id,
                name: plan.name,
                visible: plan.visible,
                opacity: plan.opacity,
                is_background: plan.is_background,
                width: size.0,
                height: size.1,
                rgba,
            },
        ));
    }
    if offset != bytes.len() {
        return Err(unsupported(
            "Trailing PDN data is not part of the qualified graph/buffers",
        ));
    }
    decoded.sort_unstable_by_key(|(index, _)| *index);
    let mut layers = vec_checked(count, limits.graph_metadata_bytes)?;
    layers.extend(decoded.into_iter().map(|(_, layer)| layer));
    check()?;
    Ok(Decoded {
        width: size.0,
        height: size.1,
        saved_with,
        header_version: inspected.hints.saved_with_version,
        layers,
        diagnostics: [Diagnostic::MetadataUnretained],
    })
}
