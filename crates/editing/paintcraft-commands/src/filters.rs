//! Strict versioned filter grammar; numeric masks remain separately admitted.
use super::*;
use paintcraft_core::types::filters::Descriptor;

pub(super) mod masks;
mod value;
pub use masks::MaskOp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterRef {
    Alias(String),
    Id(Uuid),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Mask {
        layer: LayerRef,
        filter: FilterRef,
        operation: MaskOp,
    },
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
impl FilterOp {
    pub const fn required_batch_version(&self) -> u8 {
        match self {
            Self::Mask { .. } => 3,
            Self::Add { descriptor, .. } | Self::Update { descriptor, .. } => match descriptor {
                Descriptor::LevelsRgbV1(_)
                | Descriptor::ExposureSrgbV1(_)
                | Descriptor::CurvesRgbV1(_)
                | Descriptor::VibranceRgbV1(_)
                | Descriptor::HueSaturationRgbV1(_) => 6,
                Descriptor::BoxBlurV1(_) => 5,
                Descriptor::InvertRgbV1 | Descriptor::BrightnessContrastV1(_) => 2,
                Descriptor::BlackWhiteRgbV1(_)
                | Descriptor::ThresholdLumaV1(_)
                | Descriptor::PosterizeRgbV1(_)
                | Descriptor::SolarizeRgbV1 => 4,
            },
            Self::SetEnabled { .. }
            | Self::Reorder { .. }
            | Self::Remove { .. }
            | Self::Duplicate { .. } => 2,
        }
    }
    pub fn references(&self) -> (&LayerRef, Option<&FilterRef>) {
        match self {
            Self::Add { layer, .. } => (layer, None),
            Self::Mask { layer, filter, .. }
            | Self::Update { layer, filter, .. }
            | Self::SetEnabled { layer, filter, .. }
            | Self::Reorder { layer, filter, .. }
            | Self::Remove { layer, filter }
            | Self::Duplicate { layer, filter, .. } => (layer, Some(filter)),
        }
    }
    /// Host captures must not resolve a changing active target or batch alias.
    pub fn stable_uuid_refs(&self) -> Option<(Uuid, Option<Uuid>)> {
        let (LayerRef::Id(layer), filter) = self.references() else {
            return None;
        };
        let filter = match filter {
            None => None,
            Some(FilterRef::Id(id)) => Some(*id),
            Some(FilterRef::Alias(_)) => return None,
        };
        Some((*layer, filter))
    }
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
    Descriptor::from_value(v)
}
pub(super) fn parse(v: &Value) -> Result<Op> {
    parse_using(v, descriptor)
}
pub(super) fn parse_catalog(v: &Value) -> Result<Op> {
    parse_using(v, Descriptor::from_catalog_value)
}
fn parse_using(v: &Value, descriptor: fn(&Value) -> Result<Descriptor>) -> Result<Op> {
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

pub(super) fn parse_with_coverage(
    v: &Value,
    version: u8,
    budget: &paintcraft_core::types::resources::SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Op> {
    if v.get("op")
        .and_then(Value::as_str)
        .is_some_and(|name| name.starts_with("filter.mask."))
    {
        masks::parse(v, budget, check)
    } else if version >= 6 {
        parse_catalog(v)
    } else {
        parse(v)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "filters/box_tests.rs"]
mod box_tests;
