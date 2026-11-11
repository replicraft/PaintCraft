//! Explicit versioned transaction disclosures; old result shapes stay unchanged.
use super::*;
impl Receipt {
    pub fn to_value(&self) -> Value {
        let Value::Object(mut fields) = Value::object([
            (
                "schema",
                Value::str(
                    if self.style_edit || self.document_features.contains(&"layer-styles.v1") {
                        "paintcraft.result/layer-styles-v1"
                    } else if self.document_features.contains(&"layer-blend-modes.v1") {
                        "paintcraft.result/v6"
                    } else if self.document_features.contains(&"layer-masks.v1") {
                        "paintcraft.result/v5"
                    } else if !self.required_filters.is_empty() {
                        "paintcraft.result/catalog-v1"
                    } else if self.document_features.contains(&"layer-spatial-filters.v1") {
                        "paintcraft.result/v4"
                    } else if !self.document_features.is_empty() {
                        "paintcraft.result/v3"
                    } else if self.filter_aliases.is_empty() {
                        "paintcraft.result/v1"
                    } else {
                        "paintcraft.result/v2"
                    },
                ),
            ),
            ("committed", Value::Bool(self.committed)),
            ("state_id", Value::str(self.state_id.to_string())),
            (
                "publication_epoch",
                Value::str(self.publication_epoch.to_string()),
            ),
            (
                "history_id",
                self.history_id
                    .map_or(Value::Null, |h| Value::str(h.to_string())),
            ),
            (
                "aliases",
                Value::Object(
                    self.aliases
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::str(v.to_string())))
                        .collect(),
                ),
            ),
            ("warnings", Value::Array(Vec::new())),
        ]) else {
            unreachable!("object constructor");
        };
        if self.style_edit || !self.document_features.is_empty() {
            fields.insert(
                "document_features".into(),
                Value::Array(
                    self.document_features
                        .iter()
                        .copied()
                        .map(Value::str)
                        .collect(),
                ),
            );
        }
        if self.style_edit
            || !self.required_filters.is_empty()
            || self.document_features.iter().any(|feature| {
                matches!(
                    *feature,
                    "layer-masks.v1" | "layer-blend-modes.v1" | "layer-styles.v1"
                )
            })
        {
            fields.insert(
                "required_filters".into(),
                Value::Array(
                    self.required_filters
                        .iter()
                        .copied()
                        .map(Value::str)
                        .collect(),
                ),
            );
        }
        if !self.filter_aliases.is_empty() {
            fields.insert(
                "filter_aliases".into(),
                Value::Object(
                    self.filter_aliases
                        .iter()
                        .map(|(alias, (layer, filter))| {
                            (
                                alias.clone(),
                                Value::object([
                                    ("layer_id", Value::str(layer.to_string())),
                                    ("filter_id", Value::str(filter.to_string())),
                                ]),
                            )
                        })
                        .collect(),
                ),
            );
        }
        if self.style_edit || self.document_features.contains(&"layer-styles.v1") {
            fields.insert(
                "style_aliases".into(),
                Value::Object(
                    self.style_aliases
                        .iter()
                        .map(|(alias, (layer, style))| {
                            (
                                alias.clone(),
                                Value::object([
                                    ("layer_id", Value::str(layer.to_string())),
                                    ("style_id", Value::str(style.to_string())),
                                ]),
                            )
                        })
                        .collect(),
                ),
            );
        }
        Value::Object(fields)
    }
}
