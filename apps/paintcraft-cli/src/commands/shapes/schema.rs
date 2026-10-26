//! Integer-only, lossless JSON contract for CSH inspection, not document commands.
use super::*;
use paintcraft_formats::csh::{Operation as CshOperation, PathFillRule, Record as CshRecord};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    Exclude,
    Combine,
    Subtract,
    Intersect,
}
// A required field with an explicit null distinguishes an absent source
// operation from a consumer accidentally dropping the operation property.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum OperationFlag {
    Present(Operation),
    Absent(()),
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum FillRule {
    EvenOddLegacy,
    EvenOdd,
    NonZero,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Record {
    Subpath {
        open: bool,
        knots: u16,
        operation: OperationFlag,
        fill_rule: FillRule,
    },
    Knot {
        open: bool,
        linked: bool,
        /// x0,y0,x1,y1,x2,y2; exact signed normalized 8.24 wire values.
        points_i32: [i32; 6],
    },
    FillRule {
        rule: u16,
    },
    Clipboard {
        /// top,left,bottom,right,resolution; exact signed 8.24 wire values.
        values_i32: [i32; 5],
    },
    InitialFill {
        filled: bool,
    },
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Shape {
    name: String,
    /// Original resource identifier. Duplicate IDs are retained; use ordered indices.
    id: String,
    /// Original unsigned top,left,bottom,right bounds, including the origin.
    bounds_tlbr: [u32; 4],
    /// Original record order, including fill and clipboard records.
    records: Vec<Record>,
}
#[derive(Serialize, Deserialize, JsonSchema)]
enum SchemaId {
    #[serde(rename = "paintcraft.shape-inspection/v1")]
    V1,
}
#[derive(Serialize, Deserialize, JsonSchema)]
enum ResourceType {
    #[serde(rename = "shape-library")]
    ShapeLibrary,
}
#[derive(Serialize, Deserialize, JsonSchema)]
enum CoordinateEncoding {
    #[serde(rename = "signed-normalized-8.24-i32")]
    Normalized8_24,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspection {
    schema: SchemaId,
    resource_type: ResourceType,
    #[schemars(regex(pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"))]
    resource_id: String,
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    source_sha256: String,
    coordinate_encoding: CoordinateEncoding,
    shapes: Vec<Shape>,
}
fn value(input: &impl Serialize) -> Result<Value> {
    let bytes = serde_json::to_vec(input)
        .map_err(|_| Error::new(ErrorCode::InputSchema, "cannot serialize shape inspection"))?;
    paintcraft_core::types::json::parse(&bytes)
        .map_err(|_| Error::new(ErrorCode::InputSchema, "invalid shape inspection JSON"))
}
pub(super) fn inspection(library: &Library, sha: &str) -> Result<Value> {
    let records = |source: &CshRecord| match source {
        CshRecord::Subpath {
            open,
            knots,
            operation,
            fill_rule,
        } => Record::Subpath {
            open: *open,
            knots: *knots,
            operation: operation
                .map(|op| {
                    OperationFlag::Present(match op {
                        CshOperation::Exclude => Operation::Exclude,
                        CshOperation::Combine => Operation::Combine,
                        CshOperation::Subtract => Operation::Subtract,
                        CshOperation::Intersect => Operation::Intersect,
                    })
                })
                .unwrap_or(OperationFlag::Absent(())),
            fill_rule: match fill_rule {
                PathFillRule::EvenOddLegacy => FillRule::EvenOddLegacy,
                PathFillRule::EvenOdd => FillRule::EvenOdd,
                PathFillRule::NonZero => FillRule::NonZero,
            },
        },
        CshRecord::Knot {
            open,
            linked,
            points,
        } => Record::Knot {
            open: *open,
            linked: *linked,
            points_i32: *points,
        },
        CshRecord::FillRule(rule) => Record::FillRule { rule: *rule },
        CshRecord::Clipboard(values) => Record::Clipboard {
            values_i32: *values,
        },
        CshRecord::InitialFill(filled) => Record::InitialFill { filled: *filled },
    };
    value(&Inspection {
        schema: SchemaId::V1,
        resource_type: ResourceType::ShapeLibrary,
        resource_id: deterministic_id(&["shape-library", "csh", sha]).to_string(),
        source_sha256: sha.into(),
        coordinate_encoding: CoordinateEncoding::Normalized8_24,
        shapes: library
            .shapes()
            .iter()
            .map(|shape| Shape {
                name: shape.name.clone(),
                id: shape.id.clone(),
                bounds_tlbr: shape.bounds,
                records: shape.records.iter().map(records).collect(),
            })
            .collect(),
    })
}
pub(super) fn schema() -> Result<Value> {
    let mut schema = schemars::schema_for!(Inspection);
    schema.insert(
        "$id".into(),
        serde_json::json!("urn:paintcraft:schema:shape-inspection-v1"),
    );
    value(&schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_inspection_requires_integral_i32_controls_and_complete_record_flags() {
        let library = csh::decode(
            include_bytes!(
                "../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
            ),
            &|| Ok(()),
        )
        .unwrap();
        let bytes = paintcraft_core::types::json::canonical(
            &inspection(&library, &"ab".repeat(32)).unwrap(),
        );
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(serde_json::from_value::<Inspection>(original.clone()).is_ok());
        for coordinate in [
            serde_json::json!(1.5),
            serde_json::json!(2147483648i64),
            serde_json::json!(-2147483649i64),
        ] {
            let mut bad = original.clone();
            bad["shapes"][0]["records"][4]["points_i32"][0] = coordinate;
            assert!(serde_json::from_value::<Inspection>(bad).is_err());
        }
        for key in ["open", "linked", "points_i32"] {
            let mut bad = original.clone();
            bad["shapes"][0]["records"][4]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(serde_json::from_value::<Inspection>(bad).is_err());
        }
        for key in ["open", "knots", "operation", "fill_rule"] {
            let mut bad = original.clone();
            bad["shapes"][0]["records"][3]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(serde_json::from_value::<Inspection>(bad).is_err());
        }
        let mut bounds = original.clone();
        bounds["shapes"][0]["bounds_tlbr"][0] = serde_json::json!(-1);
        assert!(serde_json::from_value::<Inspection>(bounds).is_err());
    }
}
