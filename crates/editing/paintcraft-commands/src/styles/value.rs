//! Portable typed-to-wire adapter; the caller selects the required batch profile.
use super::{StyleOp, StyleRef};
use crate::{LayerRef, Result, Value, bad};

fn layer_value(reference: &LayerRef) -> Value {
    match reference {
        LayerRef::Alias(alias) => Value::object([("ref", Value::str(alias))]),
        LayerRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
    }
}
fn style_value(reference: &StyleRef) -> Value {
    match reference {
        StyleRef::Alias(alias) => Value::object([("ref", Value::str(alias))]),
        StyleRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
    }
}
fn index_value(index: usize) -> Result<Value> {
    if index > 256 {
        return Err(bad("style index out of range"));
    }
    i64::try_from(index)
        .map(Value::Int)
        .map_err(|_| bad("style index exceeds wire integer range"))
}

impl StyleOp {
    /// Produce one strict operation object. Use `required_batch_version()` or
    /// `Batch::schema_for_operations` to select its batch envelope.
    ///
    /// This validates the same field grammar as parsing, including manually
    /// constructed aliases and final indices. It does not resolve aliases or
    /// validate indices against a particular layer's current style count.
    /// Missing optional final indices remain absent; explicit disabled state is
    /// retained. Hosts still supply the batch's base and publication epoch.
    pub fn to_value(&self) -> Result<Value> {
        let (name, layer, mut fields) = match self {
            Self::Add {
                layer,
                alias,
                descriptor,
                enabled,
                to_index,
            } => {
                let mut fields = vec![
                    ("ref", Value::str(alias)),
                    ("descriptor", descriptor.to_value()),
                    ("enabled", Value::Bool(*enabled)),
                ];
                if let Some(index) = to_index {
                    fields.push(("to_index", index_value(*index)?));
                }
                ("layer.style.add", layer, fields)
            }
            Self::Update {
                layer,
                style,
                descriptor,
            } => (
                "layer.style.update",
                layer,
                vec![
                    ("style_ref", style_value(style)),
                    ("descriptor", descriptor.to_value()),
                ],
            ),
            Self::SetEnabled {
                layer,
                style,
                enabled,
            } => (
                "layer.style.set_enabled",
                layer,
                vec![
                    ("style_ref", style_value(style)),
                    ("enabled", Value::Bool(*enabled)),
                ],
            ),
            Self::Move {
                layer,
                style,
                to_index,
            } => (
                "layer.style.move",
                layer,
                vec![
                    ("style_ref", style_value(style)),
                    ("to_index", index_value(*to_index)?),
                ],
            ),
            Self::Remove { layer, style } => (
                "layer.style.remove",
                layer,
                vec![("style_ref", style_value(style))],
            ),
            Self::Duplicate {
                layer,
                style,
                alias,
                to_index,
            } => {
                let mut fields = vec![
                    ("style_ref", style_value(style)),
                    ("ref", Value::str(alias)),
                ];
                if let Some(index) = to_index {
                    fields.push(("to_index", index_value(*index)?));
                }
                ("layer.style.duplicate", layer, fields)
            }
        };
        fields.push(("op", Value::str(name)));
        fields.push(("layer_ref", layer_value(layer)));
        let value = Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        );
        // Keep a single authority for aliases, references and descriptor bounds.
        // This is operation-only parsing and never recursively calls to_value.
        super::parse(&value)?;
        Ok(value)
    }
}
