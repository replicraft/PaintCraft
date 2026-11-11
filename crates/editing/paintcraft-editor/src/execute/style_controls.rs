//! Complete additional style-vector/alias staging under the SAME metadata scope.
//! Pure quote grants nothing. The caller's original vectors are excluded; new
//! copies remain declared through Staged until persistent/history admission.
use super::*;
use paintcraft_commands::StyleOp;
use paintcraft_core::types::resources::BudgetAccount;
use paintcraft_document::StyleInstance;
use paintcraft_raster::raster::metadata_clone::fresh_map_layout;
use std::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub bytes: u64,
    pub entries: u64,
    pub introduced: u64,
    pub aliases: usize,
}
/// Complete new query/account/move slots; not a physical stack measurement.
/// Pure scalar layout fact, not an allocation or execution grant.
pub const fn control_bytes() -> u64 {
    (2 * size_of::<Quote>()
        + 4 * size_of::<Result<Quote>>()
        + 4 * size_of::<Option<BudgetAccount>>()
        + 2 * size_of::<Result<Option<BudgetAccount>>>()
        + 2 * size_of::<BudgetAccount>()
        + 16 * size_of::<usize>()
        + 8 * size_of::<bool>()) as u64
}
/// Four full formations/moves plus256 bounded alias/layout/scalar actions.
pub const fn formation_work() -> u64 {
    4 * control_bytes() + 256
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Style staging metadata exceeds its limit",
    )
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
fn leaf() -> Result<u64> {
    // Same pinned BTree11-slot field/padding policy as compact Filter aliases.
    let fields = [
        (size_of::<usize>(), align_of::<usize>()),
        (2, 2),
        (2, 2),
        (size_of::<String>() * 11, align_of::<String>()),
        (size_of::<(Uuid, Uuid)>() * 11, align_of::<(Uuid, Uuid)>()),
    ];
    let alignment = fields.iter().map(|(_, a)| *a).max().unwrap_or(1);
    let total = fields
        .into_iter()
        .try_fold(0, |n, (w, a)| add(n, (w + a - 1) as u64))?;
    add(total, (alignment - 1) as u64)
}
/// Additional styles only, not complete existing raster/coverage metadata.
/// No allocation, factory creation, budget mutation or ambient work capture.
/// # Errors
/// Refuses arithmetic overflow, cancellation and impossible source stack bounds.
pub fn quote_checked(
    base: &Document,
    batch: &Batch,
    check: &dyn Fn() -> Result<()>,
) -> Result<Quote> {
    let mut entries = 0u64;
    for layer in base.layers.values() {
        check()?;
        if layer.styles.len() > 256 {
            return Err(limit());
        }
        entries = add(entries, layer.styles.len() as u64)?;
    }
    let mut introduced = 0;
    let mut aliases = 0usize;
    let mut style = false;
    for operation in &batch.operations {
        check()?;
        match operation {
            Op::Style(op) => {
                style = true;
                if let StyleOp::Add { alias, .. } | StyleOp::Duplicate { alias, .. } = op
                    && alias.len() > 64
                {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "Style alias exceeds64bytes",
                    ));
                }
                if matches!(op, StyleOp::Add { .. } | StyleOp::Duplicate { .. }) {
                    introduced = add(introduced, 1)?;
                    aliases = aliases.checked_add(1).ok_or_else(limit)?;
                }
            }
            Op::LayerDuplicate { .. } => introduced = add(introduced, 256)?,
            _ => {}
        }
    }
    // Empty existing documents never introduce styles through layer duplication.
    if entries == 0 && !style {
        return Ok(Quote {
            bytes: 0,
            entries,
            introduced: 0,
            aliases: 0,
        });
    }
    let vectors = mul(
        add(mul(2, add(entries, introduced)?)?, 512)?,
        size_of::<StyleInstance>() as u64,
    )?;
    let map = fresh_map_layout::<String, Option<(Uuid, Uuid)>>(aliases)?.bytes;
    let alias_bytes = add(map, mul(aliases as u64, add(leaf()?, 64)?)?)?;
    // base-state36 +layerUUID36 +operation-index4 +joined11/36/36/64/4+slashes4=155.
    let deterministic_text = 36 + 36 + 4 + 155;
    let controls = control_bytes();
    if controls > 2048 {
        return Err(limit());
    }
    Ok(Quote {
        bytes: add(
            add(add(vectors, alias_bytes)?, controls)?,
            deterministic_text,
        )?,
        entries,
        introduced,
        aliases,
    })
}
pub(super) fn admit(
    base: &Document,
    batch: &Batch,
    admission: &Admission,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<BudgetAccount>> {
    let quote = quote_checked(base, batch, check)?;
    if quote.bytes == 0 {
        return Ok(None);
    }
    if quote.bytes > admission.paint_metadata_limit() as u64 {
        return Err(limit());
    }
    let mut account = admission.paint_metadata_account();
    account.resize(quote.bytes)?;
    Ok(Some(account))
}
/// One existing-source style-vector copy for direct paste/flatten transactions.
/// Ordinary unstyled sources preserve the existing zero-account route.
/// # Errors
/// Cancellation, arithmetic overflow or the SAME caller's admission denial.
pub(crate) fn admit_document_clone(
    base: &Document,
    admission: &Admission,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<BudgetAccount>> {
    let mut bytes = 0u64;
    for layer in base.layers.values() {
        check()?;
        bytes = add(
            bytes,
            mul(layer.styles.len() as u64, size_of::<StyleInstance>() as u64)?,
        )?;
    }
    if bytes == 0 {
        return Ok(None);
    }
    if control_bytes() > 2048 {
        return Err(limit());
    }
    bytes = add(bytes, control_bytes())?;
    if bytes > admission.paint_metadata_limit() as u64 {
        return Err(limit());
    }
    let mut account = admission.paint_metadata_account();
    account.resize(bytes)?;
    Ok(Some(account))
}
pub(super) fn clone_layer(layer: &Layer, check: &dyn Fn() -> Result<()>) -> Result<Layer> {
    let styles = layer.styles.try_clone_checked(check)?;
    Ok(Layer {
        id: layer.id,
        name: layer.name.clone(),
        origin: layer.origin,
        visible: layer.visible,
        opacity: layer.opacity,
        blend_mode: layer.blend_mode,
        raster: layer.raster.clone(),
        filters: layer.filters.clone(),
        styles,
        mask: layer.mask.clone(),
    })
}
pub(crate) fn clone_document(
    base: &Document,
    styled: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    if !styled {
        return Ok(base.clone());
    }
    let mut layers = HashMap::new();
    layers.try_reserve(base.layers.len()).map_err(|_| limit())?;
    for (id, layer) in &base.layers {
        check()?;
        layers.insert(*id, clone_layer(layer, check)?);
    }
    Ok(Document {
        document_id: base.document_id,
        state_id: base.state_id,
        canvas: base.canvas,
        roots: base.roots.clone(),
        layers,
        title: base.title.clone(),
        source_metadata: base.source_metadata.clone(),
    })
}
