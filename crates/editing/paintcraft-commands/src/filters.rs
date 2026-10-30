//! Strict v2 filter grammar; selection masks are not implied by these operations.
use super::*;
use paintcraft_core::types::filters::{BrightnessContrastV1, Descriptor};

mod value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterRef {
    Alias(String),
    Id(Uuid),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Add {
        layer: LayerRef,
        alias: String,
        descriptor: Descriptor,
        enabled: bool,
        to_index: Option<usize>,
    },
    Update {
        layer: LayerRef,
        filter: FilterRef,
        descriptor: Descriptor,
    },
    SetEnabled {
        layer: LayerRef,
        filter: FilterRef,
        enabled: bool,
    },
    Reorder {
        layer: LayerRef,
        filter: FilterRef,
        to_index: usize,
    },
    Remove {
        layer: LayerRef,
        filter: FilterRef,
    },
    Duplicate {
        layer: LayerRef,
        filter: FilterRef,
        alias: String,
        to_index: Option<usize>,
    },
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| bad("enabled must be boolean"))
}
fn index(v: &Value) -> Result<usize> {
    Ok(int_in(v, 0, 256, "filter index")? as usize)
}
fn reference(v: &Value) -> Result<FilterRef> {
    // Same strict field union and identifier/alias grammar as layer references.
    Ok(match layer_ref(v)? {
        LayerRef::Alias(a) => FilterRef::Alias(a),
        LayerRef::Id(id) => FilterRef::Id(id),
    })
}
fn descriptor(v: &Value) -> Result<Descriptor> {
    let kind = v
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("filter descriptor needs kind"))?;
    match kind {
        "invert-rgb-v1" => {
            obj(v, &["kind"], &[])?;
            Ok(Descriptor::InvertRgbV1)
        }
        "brightness-contrast-v1" => {
            let o = obj(v, &["kind", "brightness", "contrast"], &[])?;
            let b = int_in(&o["brightness"], -100, 100, "brightness")? as i16;
            let c = int_in(&o["contrast"], -99, 99, "contrast")? as i16;
            Ok(Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(
                b, c,
            )?))
        }
        _ => Err(bad("unknown filter algorithm/version")),
    }
}
pub(super) fn parse(v: &Value) -> Result<Op> {
    let name = v
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("filter needs op"))?;
    let required: &[&str] = match name {
        "filter.add" => &["op", "layer_ref", "ref", "descriptor"],
        "filter.update" => &["op", "layer_ref", "filter_ref", "descriptor"],
        "filter.set_enabled" => &["op", "layer_ref", "filter_ref", "enabled"],
        "filter.reorder" => &["op", "layer_ref", "filter_ref", "to_index"],
        "filter.remove" => &["op", "layer_ref", "filter_ref"],
        "filter.duplicate" => &["op", "layer_ref", "filter_ref", "ref"],
        _ => return Err(bad("unknown filter operation")),
    };
    let optional: &[&str] = match name {
        "filter.add" => &["enabled", "to_index"],
        "filter.duplicate" => &["to_index"],
        _ => &[],
    };
    let o = obj(v, required, optional)?;
    let layer = layer_ref(&o["layer_ref"])?;
    let op = match name {
        "filter.add" => FilterOp::Add {
            layer,
            alias: alias(&o["ref"])?,
            descriptor: descriptor(&o["descriptor"])?,
            enabled: o.get("enabled").map(boolean).transpose()?.unwrap_or(true),
            to_index: o.get("to_index").map(index).transpose()?,
        },
        "filter.update" => FilterOp::Update {
            layer,
            filter: reference(&o["filter_ref"])?,
            descriptor: descriptor(&o["descriptor"])?,
        },
        "filter.set_enabled" => FilterOp::SetEnabled {
            layer,
            filter: reference(&o["filter_ref"])?,
            enabled: boolean(&o["enabled"])?,
        },
        "filter.reorder" => FilterOp::Reorder {
            layer,
            filter: reference(&o["filter_ref"])?,
            to_index: index(&o["to_index"])?,
        },
        "filter.remove" => FilterOp::Remove {
            layer,
            filter: reference(&o["filter_ref"])?,
        },
        "filter.duplicate" => FilterOp::Duplicate {
            layer,
            filter: reference(&o["filter_ref"])?,
            alias: alias(&o["ref"])?,
            to_index: o.get("to_index").map(index).transpose()?,
        },
        _ => unreachable!("validated operation"),
    };
    Ok(Op::Filter(op))
}

#[cfg(test)]
mod tests;
