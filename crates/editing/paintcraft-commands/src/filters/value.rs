//! Portable typed-to-wire adapter for the existing strict v2 grammar.
use super::{FilterOp, FilterRef};
use crate::{LayerRef, Result, Value, bad};

fn layer_value(reference: &LayerRef) -> Value {
    match reference {
        LayerRef::Alias(alias) => Value::object([("ref", Value::str(alias))]),
        LayerRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
    }
}
fn filter_value(reference: &FilterRef) -> Value {
    match reference {
        FilterRef::Alias(alias) => Value::object([("ref", Value::str(alias))]),
        FilterRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
    }
}
fn index_value(index: usize) -> Result<Value> {
    if index > 256 {
        return Err(bad("filter index out of range"));
    }
    i64::try_from(index)
        .map(Value::Int)
        .map_err(|_| bad("filter index exceeds wire integer range"))
}

impl FilterOp {
    /// Produce one operation object for `paintcraft.batch/v2`.
    ///
    /// This validates the same field grammar as parsing, including manually
    /// constructed aliases and final indices. It does not resolve aliases or
    /// validate indices against a particular layer's current filter count.
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
                ("filter.add", layer, fields)
            }
            Self::Update {
                layer,
                filter,
                descriptor,
            } => (
                "filter.update",
                layer,
                vec![
                    ("filter_ref", filter_value(filter)),
                    ("descriptor", descriptor.to_value()),
                ],
            ),
            Self::SetEnabled {
                layer,
                filter,
                enabled,
            } => (
                "filter.set_enabled",
                layer,
                vec![
                    ("filter_ref", filter_value(filter)),
                    ("enabled", Value::Bool(*enabled)),
                ],
            ),
            Self::Reorder {
                layer,
                filter,
                to_index,
            } => (
                "filter.reorder",
                layer,
                vec![
                    ("filter_ref", filter_value(filter)),
                    ("to_index", index_value(*to_index)?),
                ],
            ),
            Self::Remove { layer, filter } => (
                "filter.remove",
                layer,
                vec![("filter_ref", filter_value(filter))],
            ),
            Self::Duplicate {
                layer,
                filter,
                alias,
                to_index,
            } => {
                let mut fields = vec![
                    ("filter_ref", filter_value(filter)),
                    ("ref", Value::str(alias)),
                ];
                if let Some(index) = to_index {
                    fields.push(("to_index", index_value(*index)?));
                }
                ("filter.duplicate", layer, fields)
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

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
