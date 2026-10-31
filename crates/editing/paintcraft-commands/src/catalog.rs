//! Explicit catalog envelope. Frozen numeric batch grammars remain closed.
use super::*;

pub const CATALOG_BATCH_SCHEMA: &str = "paintcraft.batch/catalog-v1";

const PROFILES: [&str; 8] = [
    "black-white-rgb-v1",
    "box-blur-v1",
    "brightness-contrast-v1",
    "invert-rgb-v1",
    "levels-rgb-v1",
    "posterize-rgb-v1",
    "solarize-rgb-v1",
    "threshold-luma-v1",
];

impl Batch {
    /// Exact sorted numerical requirements of descriptor-bearing operations.
    /// This bounded projection grants no parameter or execution authority.
    pub fn catalog_requirements_for_fields<'a>(
        fields: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    ) -> Result<Vec<&'static str>> {
        let mut requirements = Vec::new();
        for (index, (name, kind)) in fields.into_iter().enumerate() {
            if index >= MAX_OPERATIONS {
                return Err(bad("operations must contain 1..=1024 items"));
            }
            if !matches!(name, "filter.add" | "filter.update") {
                continue;
            }
            let profile = kind
                .and_then(|kind| PROFILES.iter().copied().find(|known| *known == kind))
                .ok_or_else(|| bad("Unknown catalog numerical profile"))?;
            if !requirements.contains(&profile) {
                requirements.push(profile);
            }
        }
        requirements.sort_unstable();
        Ok(requirements)
    }

    /// Add requirements only to an internally generated catalog envelope, then
    /// run the same strict public parser. Untrusted callers use from_value.
    pub fn from_generated_value(mut value: Value) -> Result<Self> {
        if value.get("schema").and_then(Value::as_str) == Some(CATALOG_BATCH_SCHEMA) {
            let requirements = required(&value)?;
            let Value::Object(ref mut fields) = value else {
                return Err(bad("expected a batch object"));
            };
            if fields.contains_key("required_filters") {
                return Err(bad("generated batch already declares required filters"));
            }
            fields.insert(
                "required_filters".into(),
                Value::Array(requirements.into_iter().map(Value::str).collect()),
            );
        }
        Self::from_value(&value)
    }
}

fn required(value: &Value) -> Result<Vec<&'static str>> {
    let operations = value
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("operations must be an array"))?;
    Batch::catalog_requirements_for_fields(operations.iter().map(|operation| {
        (
            operation.get("op").and_then(Value::as_str).unwrap_or(""),
            operation
                .get("descriptor")
                .and_then(|descriptor| descriptor.get("kind"))
                .and_then(Value::as_str),
        )
    }))
}

pub(super) fn validate(value: &Value) -> Result<()> {
    let expected = required(value)?;
    let declared = value
        .get("required_filters")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("catalog batch needs required_filters"))?;
    if declared.len() != expected.len()
        || declared
            .iter()
            .zip(expected)
            .any(|(declared, expected)| declared.as_str() != Some(expected))
    {
        return Err(bad(
            "catalog requirements must equal the sorted operation inventory",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
