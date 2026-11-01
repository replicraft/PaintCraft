//! Four source-preserving operations on the owning layer's saved A8 slot.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerMaskOp {
    FromSelection { layer: LayerRef, enabled: bool },
    SetEnabled { layer: LayerRef, enabled: bool },
    Invert { layer: LayerRef },
    Remove { layer: LayerRef },
}
impl LayerMaskOp {
    pub const fn required_batch_version(&self) -> u8 {
        7
    }
    pub fn references(&self) -> &LayerRef {
        match self {
            Self::FromSelection { layer, .. }
            | Self::SetEnabled { layer, .. }
            | Self::Invert { layer }
            | Self::Remove { layer } => layer,
        }
    }
    /// Interactive hosts must freeze an actual owning layer, not a mutable alias.
    pub fn stable_uuid_ref(&self) -> Option<Uuid> {
        match self.references() {
            LayerRef::Id(id) => Some(*id),
            LayerRef::Alias(_) => None,
        }
    }
    pub fn to_value(&self) -> Result<Value> {
        let (name, enabled) = match self {
            Self::FromSelection { enabled, .. } => ("layer.mask.from_selection", Some(*enabled)),
            Self::SetEnabled { enabled, .. } => ("layer.mask.set_enabled", Some(*enabled)),
            Self::Invert { .. } => ("layer.mask.invert", None),
            Self::Remove { .. } => ("layer.mask.remove", None),
        };
        let reference = match self.references() {
            LayerRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
            LayerRef::Alias(alias) => Value::object([("ref", Value::str(alias))]),
        };
        let mut fields = BTreeMap::from([
            ("op".to_owned(), Value::str(name)),
            ("layer_ref".to_owned(), reference),
        ]);
        if let Some(enabled) = enabled {
            fields.insert("enabled".into(), Value::Bool(enabled));
        }
        let value = Value::Object(fields);
        parse(&value)?;
        Ok(value)
    }
}
pub(super) fn parse(v: &Value) -> Result<Op> {
    let name = v
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("layer mask operation needs op"))?;
    let enabled = matches!(name, "layer.mask.from_selection" | "layer.mask.set_enabled");
    let required: &[&str] = if enabled {
        &["op", "layer_ref", "enabled"]
    } else {
        &["op", "layer_ref"]
    };
    let o = obj(v, required, &[])?;
    let layer = layer_ref(&o["layer_ref"])?;
    let enabled = if enabled {
        Some(
            o["enabled"]
                .as_bool()
                .ok_or_else(|| bad("enabled must be boolean"))?,
        )
    } else {
        None
    };
    Ok(Op::LayerMask(match name {
        "layer.mask.from_selection" => LayerMaskOp::FromSelection {
            layer,
            enabled: enabled.expect("required bool"),
        },
        "layer.mask.set_enabled" => LayerMaskOp::SetEnabled {
            layer,
            enabled: enabled.expect("required bool"),
        },
        "layer.mask.invert" => LayerMaskOp::Invert { layer },
        "layer.mask.remove" => LayerMaskOp::Remove { layer },
        _ => return Err(bad("unknown layer mask operation")),
    }))
}
#[cfg(test)]
mod tests;
