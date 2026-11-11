//! Closed style operation grammar; no numerical filter or coverage namespace.
use super::*;
use paintcraft_core::types::styles::StyleDescriptor as Descriptor;

mod value;
pub const LAYER_STYLES_BATCH_SCHEMA: &str = "paintcraft.batch/layer-styles-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleRef {
    Alias(String),
    Id(Uuid),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleOp {
    Add {
        layer: LayerRef,
        alias: String,
        descriptor: Descriptor,
        enabled: bool,
        to_index: Option<usize>,
    },
    Update {
        layer: LayerRef,
        style: StyleRef,
        descriptor: Descriptor,
    },
    SetEnabled {
        layer: LayerRef,
        style: StyleRef,
        enabled: bool,
    },
    Move {
        layer: LayerRef,
        style: StyleRef,
        to_index: usize,
    },
    Remove {
        layer: LayerRef,
        style: StyleRef,
    },
    Duplicate {
        layer: LayerRef,
        style: StyleRef,
        alias: String,
        to_index: Option<usize>,
    },
}
impl StyleOp {
    pub const fn required_batch_version(&self) -> u8 {
        10
    }
    pub fn references(&self) -> (&LayerRef, Option<&StyleRef>) {
        match self {
            Self::Add { layer, .. } => (layer, None),
            Self::Update { layer, style, .. }
            | Self::SetEnabled { layer, style, .. }
            | Self::Move { layer, style, .. }
            | Self::Remove { layer, style }
            | Self::Duplicate { layer, style, .. } => (layer, Some(style)),
        }
    }
    /// Host captures must not resolve a changing active target or batch alias.
    pub fn stable_uuid_refs(&self) -> Option<(Uuid, Option<Uuid>)> {
        let (LayerRef::Id(layer), style) = self.references() else {
            return None;
        };
        let style = match style {
            None => None,
            Some(StyleRef::Id(id)) => Some(*id),
            Some(StyleRef::Alias(_)) => return None,
        };
        Some((*layer, style))
    }
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| bad("enabled must be boolean"))
}
fn index(v: &Value) -> Result<usize> {
    Ok(int_in(v, 0, 256, "style index")? as usize)
}
fn reference(v: &Value) -> Result<StyleRef> {
    // Same strict field union and identifier/alias grammar as layer references.
    Ok(match layer_ref(v)? {
        LayerRef::Alias(a) => StyleRef::Alias(a),
        LayerRef::Id(id) => StyleRef::Id(id),
    })
}
fn descriptor(v: &Value) -> Result<Descriptor> {
    Descriptor::from_value(v)
}
pub(super) fn parse(v: &Value) -> Result<Op> {
    parse_using(v, descriptor)
}
fn parse_using(v: &Value, descriptor: fn(&Value) -> Result<Descriptor>) -> Result<Op> {
    let name = v
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("style needs op"))?;
    let required: &[&str] = match name {
        "layer.style.add" => &["op", "layer_ref", "ref", "descriptor"],
        "layer.style.update" => &["op", "layer_ref", "style_ref", "descriptor"],
        "layer.style.set_enabled" => &["op", "layer_ref", "style_ref", "enabled"],
        "layer.style.move" => &["op", "layer_ref", "style_ref", "to_index"],
        "layer.style.remove" => &["op", "layer_ref", "style_ref"],
        "layer.style.duplicate" => &["op", "layer_ref", "style_ref", "ref"],
        _ => return Err(bad("unknown style operation")),
    };
    let optional: &[&str] = match name {
        "layer.style.add" => &["enabled", "to_index"],
        "layer.style.duplicate" => &["to_index"],
        _ => &[],
    };
    let o = obj(v, required, optional)?;
    let layer = layer_ref(&o["layer_ref"])?;
    let op = match name {
        "layer.style.add" => StyleOp::Add {
            layer,
            alias: alias(&o["ref"])?,
            descriptor: descriptor(&o["descriptor"])?,
            enabled: o.get("enabled").map(boolean).transpose()?.unwrap_or(true),
            to_index: o.get("to_index").map(index).transpose()?,
        },
        "layer.style.update" => StyleOp::Update {
            layer,
            style: reference(&o["style_ref"])?,
            descriptor: descriptor(&o["descriptor"])?,
        },
        "layer.style.set_enabled" => StyleOp::SetEnabled {
            layer,
            style: reference(&o["style_ref"])?,
            enabled: boolean(&o["enabled"])?,
        },
        "layer.style.move" => StyleOp::Move {
            layer,
            style: reference(&o["style_ref"])?,
            to_index: index(&o["to_index"])?,
        },
        "layer.style.remove" => StyleOp::Remove {
            layer,
            style: reference(&o["style_ref"])?,
        },
        "layer.style.duplicate" => StyleOp::Duplicate {
            layer,
            style: reference(&o["style_ref"])?,
            alias: alias(&o["ref"])?,
            to_index: o.get("to_index").map(index).transpose()?,
        },
        _ => unreachable!("validated operation"),
    };
    Ok(Op::Style(op))
}

pub(super) fn required(value: &Value) -> Result<Vec<&'static str>> {
    let mut required = super::layer_blends::required(value)?;
    let operations = value
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("operations must be an array"))?;
    if operations.iter().any(|op| {
        op.get("op")
            .and_then(Value::as_str)
            .is_some_and(|n| n.starts_with("layer.style."))
    }) {
        required.push("layer-styles.v1");
    }
    required.sort_unstable();
    Ok(required)
}
pub(super) fn validate(value: &Value) -> Result<()> {
    let required = required(value)?;
    let got = value
        .get("required_layer_features")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("style batch requires required_layer_features"))?;
    if got.len() != required.len()
        || got
            .iter()
            .zip(required)
            .any(|(got, want)| got.as_str() != Some(want))
    {
        return Err(bad(
            "style requirements must equal sorted operation inventory",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
