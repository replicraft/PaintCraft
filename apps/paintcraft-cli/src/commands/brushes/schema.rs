//! Typed JSON inspection and its discoverable schema; not an image document.
use super::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, JsonSchema)]
enum SchemaId {
    #[serde(rename = "paintcraft.brush-inspection/v1")]
    V1,
}
#[derive(Serialize, Deserialize, JsonSchema)]
enum Resource {
    #[serde(rename = "brush-library")]
    Brushes,
}
#[derive(Serialize, Deserialize, JsonSchema)]
enum Policy {
    #[serde(rename = "native-grid-v1")]
    NativeGrid,
}
// Required nullable fields distinguish unsupported/unknown from an omitted key.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum Nullable<T> {
    Value(T),
    Null(()),
}
impl<T> From<Option<T>> for Nullable<T> {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null(()), Self::Value)
    }
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Preset {
    index: usize,
    name: String,
    qualified: bool,
    dimensions: Nullable<[u32; 2]>,
    spacing_q8: Nullable<u32>,
    unsupported: Vec<String>,
    required_conversions: Vec<String>,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspection {
    schema: SchemaId,
    resource_type: Resource,
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    source_sha256: String,
    policy: Policy,
    rendering: String,
    #[schemars(range(max = 256))]
    samples: usize,
    #[schemars(range(max = 64))]
    patterns: usize,
    #[schemars(length(max = 256))]
    presets: Vec<Preset>,
}
pub(super) fn inspection(loaded: &Loaded) -> Result<serde_json::Value> {
    // Assess re-reads bounded original descriptor metadata for fail-closed
    // feature classification. Bound this repeated work before the first scan.
    if loaded.source.brushes().len() > adoption::PRESETS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "inspection allows at most 256 brush presets",
        ));
    }
    let presets = loaded
        .source
        .brushes()
        .iter()
        .enumerate()
        .map(|(index, brush)| {
            let assessment = adoption::assess(&loaded.source, index, &|| Ok(()))?;
            Ok(Preset {
                index,
                name: brush.name.clone(),
                qualified: assessment.qualified(),
                dimensions: assessment.dimensions().map(|(w, h)| [w, h]).into(),
                spacing_q8: assessment.spacing_q8().into(),
                unsupported: assessment
                    .unsupported()
                    .iter()
                    .map(|feature| format!("{feature:?}"))
                    .collect(),
                required_conversions: assessment
                    .required_conversions()
                    .iter()
                    .map(|feature| format!("{feature:?}"))
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    serde_json::to_value(Inspection {
        schema: SchemaId::V1,
        resource_type: Resource::Brushes,
        source_sha256: loaded.sha.clone(),
        policy: Policy::NativeGrid,
        rendering: "PaintCraft static native R8; unsupported preset behavior is refused".into(),
        samples: loaded.source.samples().len(),
        patterns: loaded.source.patterns().len(),
        presets,
    })
    .map_err(|_| Error::new(ErrorCode::InputSchema, "cannot serialize brush inspection"))
}
pub(super) fn schema() -> Result<serde_json::Value> {
    serde_json::to_value(schemars::schema_for!(Inspection))
        .map_err(|_| Error::new(ErrorCode::InputSchema, "cannot serialize brush schema"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_inspection_requires_ids_integral_geometry_and_nullable_fields() {
        let original = serde_json::json!({"schema":"paintcraft.brush-inspection/v1","resource_type":"brush-library","source_sha256":"0".repeat(64),"policy":"native-grid-v1","rendering":"static", "samples":0,"patterns":0,
            "presets":[{"index":0,"name":"brush","qualified":false,"dimensions":null,"spacing_q8":null,"unsupported":["MissingSample"],"required_conversions":[]}]});
        assert!(serde_json::from_value::<Inspection>(original.clone()).is_ok());
        for key in [
            "dimensions",
            "spacing_q8",
            "unsupported",
            "required_conversions",
        ] {
            let mut value = original.clone();
            value["presets"][0].as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<Inspection>(value).is_err());
        }
        for value in [
            serde_json::json!([-1, 2]),
            serde_json::json!([1.5, 2]),
            serde_json::json!([1, 2, 3]),
        ] {
            let mut bad = original.clone();
            bad["presets"][0]["dimensions"] = value;
            assert!(serde_json::from_value::<Inspection>(bad).is_err());
        }
        let mut bad = original;
        bad["schema"] = serde_json::json!("wrong");
        assert!(serde_json::from_value::<Inspection>(bad).is_err());
        assert_eq!(schema().unwrap()["properties"]["presets"]["maxItems"], 256);
    }
}
