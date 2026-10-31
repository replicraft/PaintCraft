//! Typed atomic batches v1–v4. Parsing enforces each grammar profile exactly:
//! unknown fields, wrong types, out-of-range numbers and malformed patterns are
//! all `input.schema`. Semantic checks (aliases, targets) happen at execution.

use std::collections::BTreeMap;

use paintcraft_core::types::json::{self, Value};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use paintcraft_core::types::ids::parse_uuid;
use paintcraft_core::types::policy::{COORD_ABS_MAX, hex};

mod filters;
pub use filters::{FilterOp, FilterRef, MaskOp};
mod catalog;
pub use catalog::CATALOG_BATCH_SCHEMA;
mod mask;
pub use mask::{validate_mask_spans, validate_mask_spans_checked};
mod contour;
mod operations;
mod stroke;
use operations::op;

const SIZE_MAX: i64 = 1_048_576;
const MAX_OPERATIONS: usize = 1024;
const MAX_POINTS: usize = 65_536;
pub const BRUSH_SIZE_MAX: i64 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerRef {
    Alias(String),
    Id(Uuid),
}

pub use paintcraft_core::types::canvas::Anchor;

pub use paintcraft_core::types::policy::BlendMode as Mode;
pub use paintcraft_core::types::policy::MaskMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
    Preset(paintcraft_core::types::shapes::ShapePreset),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Filter(FilterOp),
    /// v2: removal half of Cut; hosts must acknowledge their clipboard write first.
    /// Clear only canvas/layer/selection coverage and preserve the selection.
    CutSelection {
        layer: LayerRef,
        clear: Rgba,
    },
    /// v2: cut selected pixels and paste an exact frozen capture at an offset.
    /// RGB-keyed samples are still cut, but skipped during the paste phase.
    MoveSelection {
        layer: LayerRef,
        offset: (i64, i64),
        clear: Rgba,
        transparent_key: Option<Rgba>,
    },
    TransformSelection {
        layer: LayerRef,
        transform: paintcraft_core::types::orthogonal::OrthogonalTransform,
        clear: Rgba,
    },
    CanvasTransform {
        transform: paintcraft_core::types::orthogonal::OrthogonalTransform,
    },
    CanvasCrop {
        rect: Rect,
    },
    Contour {
        layer: LayerRef,
        points: Vec<(i64, i64)>,
        closed: bool,
        stroke: Option<(Rgba, u32)>,
        fill: Option<Rgba>,
        mode: Mode,
    },
    LayerAdd {
        alias: String,
        name: String,
        width: u32,
        height: u32,
        origin: (i64, i64),
        default: Rgba,
    },
    /// v2: complete independent copy immediately above the source.
    LayerDuplicate {
        layer: LayerRef,
        alias: String,
        name: String,
    },
    LayerDelete {
        layer: LayerRef,
    },
    LayerSetProperties {
        layer: LayerRef,
        name: Option<String>,
        visible: Option<bool>,
        opacity: Option<u8>,
    },
    FillRect {
        layer: LayerRef,
        rect: Rect,
        color: Rgba,
        mode: Mode,
    },
    PencilPath {
        layer: LayerRef,
        points: Vec<(i64, i64)>,
        color: Rgba,
        mode: Mode,
    },
    SelectionSetRect {
        rect: Rect,
    },
    SelectionClear,
    /// v2: finite canvas-domain binary polygon at pixel centers.
    SelectionSetPolygon {
        points: Vec<(i64, i64)>,
    },
    /// v2: complement the current coverage within the canvas domain.
    SelectionInvert,
    /// v2: a brush stroke of `size` pixels along a path, blended once per pixel.
    StrokePath {
        media: paintcraft_core::types::brush::BrushMedia,
        layer: LayerRef,
        points: Vec<(i64, i64)>,
        color: Rgba,
        mode: Mode,
        size: u32,
        round: bool,
        smooth: bool,
        /// 1: integer pixel centers; 256: fixed-point document coordinates.
        point_scale: u32,
    },
    /// v2: 4-connected fill from a seed pixel.
    FloodFill {
        layer: LayerRef,
        at: (i64, i64),
        color: Rgba,
        mode: Mode,
        tolerance: u8,
    },
    /// v2: line, rectangle or ellipse between two corner pixels (inclusive).
    Shape {
        layer: LayerRef,
        kind: ShapeKind,
        from: (i64, i64),
        to: (i64, i64),
        stroke: Option<(Rgba, u32)>,
        fill: Option<Rgba>,
        mode: Mode,
    },
    /// v2: move a layer within the stack; index 0 is the bottom layer.
    LayerReorder {
        layer: LayerRef,
        to_index: usize,
    },
    /// v2: resolved grayscale text/paint coverage; optional mode defaults to source-over.
    /// Replace interpolates all straight RGBA channels using exact 8-bit coverage.
    MaskSpans {
        layer: LayerRef,
        color: Rgba,
        mode: MaskMode,
        spans: Vec<paintcraft_core::types::text::MaskSpan>,
    },
    /// v2: text in the built-in bitmap font, laid out from its top-left corner.
    Text {
        layer: LayerRef,
        at: (i64, i64),
        text: String,
        scale: u32,
        color: Rgba,
        mode: Mode,
    },
    LayerExpand {
        layer: LayerRef,
        width: u32,
        height: u32,
    },
    /// v2: replace all layers with one full-canvas background; clear selection.
    CanvasClear {
        alias: String,
        color: Rgba,
    },
    CanvasResize {
        width: u32,
        height: u32,
        anchor: Anchor,
    },
}

#[derive(Clone, Debug)]
pub struct Batch {
    /// v1 legacy; v2 drawing/initial filters; v3 masks; v4 additional point descriptors.
    pub version: u8,
    pub policy: String,
    pub base_state: Uuid,
    pub base_epoch: u64,
    pub seed: Option<u64>,
    pub operations: Vec<Op>,
    /// Lowercase hex SHA-256 of the canonical JSON of the batch as submitted.
    pub digest: String,
}

fn bad(m: impl Into<String>) -> Error {
    Error::new(ErrorCode::InputSchema, m)
}

impl Batch {
    /// Select the lowest grammar profile for a generated packet. Parsing still
    /// validates all names, fields and counts; this helper grants no authority.
    pub fn schema_for_operations(operations: &[Value]) -> &'static str {
        Self::schema_for_operation_fields(operations.iter().map(|value| {
            (
                value.get("op").and_then(Value::as_str).unwrap_or(""),
                value
                    .get("descriptor")
                    .and_then(|d| d.get("kind"))
                    .and_then(Value::as_str),
            )
        }))
    }
    /// Allocation-free selector for bounded caller-owned JSON projections,
    /// including hosts using another JSON type. It never validates or authorizes
    /// the operations; strict parsing still rejects unknown or malformed fields.
    pub fn schema_for_operation_fields<'a>(
        operations: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    ) -> &'static str {
        let version = operations
            .into_iter()
            .map(|(name, kind)| {
                if matches!(name, "filter.add" | "filter.update")
                    && matches!(
                        kind,
                        Some(
                            "levels-rgb-v1"
                                | "exposure-srgb-v1"
                                | "curves-rgb-v1"
                                | "vibrance-rgb-v1"
                        )
                    )
                {
                    6
                } else if matches!(name, "filter.add" | "filter.update")
                    && kind == Some("box-blur-v1")
                {
                    5
                } else if matches!(name, "filter.add" | "filter.update")
                    && matches!(
                        kind,
                        Some(
                            "black-white-rgb-v1"
                                | "threshold-luma-v1"
                                | "posterize-rgb-v1"
                                | "solarize-rgb-v1"
                        )
                    )
                {
                    4
                } else if name.starts_with("filter.mask.") {
                    3
                } else {
                    2
                }
            })
            .max()
            .unwrap_or(2);
        match version {
            6 => CATALOG_BATCH_SCHEMA,
            5 => "paintcraft.batch/v5",
            4 => "paintcraft.batch/v4",
            3 => "paintcraft.batch/v3",
            _ => "paintcraft.batch/v2",
        }
    }
    pub fn from_json(bytes: &[u8]) -> Result<Batch> {
        let value = json::parse(bytes).map_err(|e| bad(format!("invalid JSON: {e}")))?;
        Batch::from_value(&value)
    }

    pub fn from_value(v: &Value) -> Result<Batch> {
        Self::from_value_with_coverage(
            v,
            &paintcraft_core::types::resources::default_coverage_budget(),
            &|| Ok(()),
        )
    }

    /// Only numeric mask DTO storage is admitted here. The input JSON Value and
    /// canonical digest buffer remain caller-owned transport allocations.
    pub fn from_value_with_coverage(
        v: &Value,
        budget: &paintcraft_core::types::resources::SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Batch> {
        check()?;
        let catalog = v.get("schema").and_then(Value::as_str) == Some(CATALOG_BATCH_SCHEMA);
        let required: &[&str] = if catalog {
            &[
                "schema",
                "atomic",
                "policy",
                "base",
                "operations",
                "required_filters",
            ]
        } else {
            &["schema", "atomic", "policy", "base", "operations"]
        };
        let top = obj(v, required, &["seed"])?;
        let version = match top["schema"].as_str() {
            Some("paintcraft.batch/v1") => 1,
            Some("paintcraft.batch/v2") => 2,
            Some("paintcraft.batch/v3") => 3,
            Some("paintcraft.batch/v4") => 4,
            Some("paintcraft.batch/v5") => 5,
            Some(CATALOG_BATCH_SCHEMA) => 6,
            _ => {
                return Err(bad(
                    "schema must be paintcraft.batch/v1, paintcraft.batch/v2, paintcraft.batch/v3, paintcraft.batch/v4 or paintcraft.batch/v5",
                ));
            }
        };
        if top["atomic"].as_bool() != Some(true) {
            return Err(bad("atomic must be true"));
        }
        let policy = string(&top["policy"], "policy")?;
        if !policy_grammar(&policy) {
            return Err(bad("malformed policy identifier"));
        }
        let base = obj(&top["base"], &["state_id", "publication_epoch"], &[])?;
        let base_state = uuid(&base["state_id"], "base.state_id")?;
        let base_epoch = epoch(&base["publication_epoch"])?;
        let seed = match top.get("seed") {
            None => None,
            Some(s) => Some(int_in(s, 0, 9_007_199_254_740_991, "seed")? as u64),
        };
        let ops = top["operations"]
            .as_array()
            .ok_or_else(|| bad("operations must be an array"))?;
        if ops.is_empty() || ops.len() > MAX_OPERATIONS {
            return Err(bad("operations must contain 1..=1024 items"));
        }
        if catalog {
            catalog::validate(v)?;
        }
        let operations = ops
            .iter()
            .map(|o| {
                check()?;
                op(o, version, budget, check)
            })
            .collect::<Result<Vec<_>>>()?;
        let digest = hex(&Sha256::digest(json::canonical(v)));
        Ok(Batch {
            version,
            policy,
            base_state,
            base_epoch,
            seed,
            operations,
            digest,
        })
    }
}

/// Exactly the required keys plus any of the optional ones.
fn obj<'a>(
    v: &'a Value,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a BTreeMap<String, Value>> {
    let m = v.as_object().ok_or_else(|| bad("expected an object"))?;
    if let Some(k) = required.iter().find(|k| !m.contains_key(**k)) {
        return Err(bad(format!("missing required property {k:?}")));
    }
    if let Some(k) = m
        .keys()
        .find(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(bad(format!("unknown property {k:?}")));
    }
    Ok(m)
}

fn string(v: &Value, what: &str) -> Result<String> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| bad(format!("{what} must be a string")))
}

fn name_str(v: &Value) -> Result<String> {
    let s = string(v, "name")?;
    if s.chars().count() > 1024 {
        return Err(bad("name exceeds 1024 characters"));
    }
    Ok(s)
}

fn int_in(v: &Value, lo: i64, hi: i64, what: &str) -> Result<i64> {
    match v.as_i64() {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(bad(format!("{what} must be an integer in {lo}..={hi}"))),
    }
}

fn point(v: &Value) -> Result<(i64, i64)> {
    let o = obj(v, &["x", "y"], &[])?;
    Ok((
        int_in(&o["x"], -COORD_ABS_MAX, COORD_ABS_MAX, "x")?,
        int_in(&o["y"], -COORD_ABS_MAX, COORD_ABS_MAX, "y")?,
    ))
}

fn rect(v: &Value) -> Result<Rect> {
    let o = obj(v, &["x", "y", "width", "height"], &[])?;
    let (x, y) = (
        int_in(&o["x"], -COORD_ABS_MAX, COORD_ABS_MAX, "x")?,
        int_in(&o["y"], -COORD_ABS_MAX, COORD_ABS_MAX, "y")?,
    );
    let (w, h) = (
        int_in(&o["width"], 1, SIZE_MAX, "width")?,
        int_in(&o["height"], 1, SIZE_MAX, "height")?,
    );
    Rect::new(x, y, w, h).ok_or_else(|| bad("rectangle overflows"))
}

fn color(v: &Value) -> Result<Rgba> {
    v.as_str()
        .and_then(Rgba::parse)
        .ok_or_else(|| bad("color must be #RRGGBBAA"))
}

fn mode(v: &Value) -> Result<Mode> {
    match v.as_str() {
        Some("replace") => Ok(Mode::Replace),
        Some("source-over") => Ok(Mode::SourceOver),
        _ => Err(bad("mode must be replace or source-over")),
    }
}

fn is_alias(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=64).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_' || *c == b'-')
}

fn alias(v: &Value) -> Result<String> {
    let s = string(v, "ref")?;
    if is_alias(&s) {
        Ok(s)
    } else {
        Err(bad("malformed alias"))
    }
}

fn uuid(v: &Value, what: &str) -> Result<Uuid> {
    v.as_str()
        .and_then(parse_uuid)
        .ok_or_else(|| bad(format!("{what} must be a UUID")))
}

fn layer_ref(v: &Value) -> Result<LayerRef> {
    let m = v
        .as_object()
        .ok_or_else(|| bad("layer_ref must be an object"))?;
    match (m.len(), m.get("ref"), m.get("id")) {
        (1, Some(r), None) => Ok(LayerRef::Alias(alias(r)?)),
        (1, None, Some(i)) => Ok(LayerRef::Id(uuid(i, "layer_ref.id")?)),
        _ => Err(bad("layer_ref needs exactly one of ref or id")),
    }
}

fn epoch(v: &Value) -> Result<u64> {
    let s = v
        .as_str()
        .ok_or_else(|| bad("publication_epoch must be a decimal string"))?;
    let canonical = s == "0"
        || (s.len() <= 20
            && !s.starts_with('0')
            && !s.is_empty()
            && s.bytes().all(|c| c.is_ascii_digit()));
    if !canonical {
        return Err(bad("publication_epoch must be a canonical decimal string"));
    }
    s.parse::<u64>()
        .map_err(|_| bad("publication epoch exceeds u64"))
}

/// `^paintcraft\.[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)*/[a-z0-9][a-z0-9._-]{0,95}$`
fn policy_grammar(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("paintcraft.") else {
        return false;
    };
    let Some((family, revision)) = rest.split_once('/') else {
        return false;
    };
    let seg = |p: &str| {
        let b = p.as_bytes();
        !b.is_empty()
            && b[0].is_ascii_lowercase()
            && b.iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    let rb = revision.as_bytes();
    family.split('.').all(seg)
        && (1..=96).contains(&rb.len())
        && (rb[0].is_ascii_lowercase() || rb[0].is_ascii_digit())
        && rb.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#""schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{"state_id":"11111111-1111-4111-8111-111111111111","publication_epoch":"0"}"#;

    fn batch(ops: &str) -> Result<Batch> {
        Batch::from_json(format!("{{{BASE},\"operations\":{ops}}}").as_bytes())
    }

    const ADD: &str = r##"{"op":"layer.add","ref":"ink","name":"Ink","width":32,"height":32,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}"##;

    #[test]
    fn parses_a_valid_batch_and_digests_its_canonical_form() {
        let b = batch(&format!("[{ADD},{{\"op\":\"selection.clear\"}}]")).unwrap();
        assert_eq!(b.operations.len(), 2);
        assert_eq!((b.base_epoch, b.seed), (0, None));
        assert_eq!(b.digest.len(), 64);
        // Key order and whitespace do not change the digest.
        let reordered = Batch::from_json(
            format!("{{ \"operations\" : [{ADD},{{\"op\":\"selection.clear\"}}], {BASE} }}")
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(reordered.digest, b.digest);
    }

    #[test]
    fn rejects_schema_violations() {
        for ops in [
            "[]".to_string(),
            format!("[{}]", ADD.replace("\"width\":32", "\"width\":0")),
            format!("[{}]", ADD.replace("\"ink\"", "\"Ink\"")),
            format!("[{}]", ADD.replace("#00000000", "#000000")),
            format!("[{}]", ADD.replace("\"origin\":{\"x\":0,\"y\":0}", "\"origin\":{\"x\":140737488355328,\"y\":0}")),
            format!("[{}]", ADD.replace("\"name\":\"Ink\",", "\"name\":\"Ink\",\"extra\":1,")),
            r#"[{"op":"layer.delete","layer_ref":{"ref":"a","id":"11111111-1111-4111-8111-111111111111"}}]"#.to_string(),
            r#"[{"op":"layer.delete","layer_ref":{"ref":"a"}, "x":1}]"#.to_string(),
            r#"[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":0}}]"#.to_string(),
            r#"[{"op":"canvas.resize","width":1,"height":1,"anchor":"middle","outside_content":"preserve"}]"#.to_string(),
            r#"[{"op":"canvas.resize","width":1,"height":1,"anchor":"top","outside_content":"crop"}]"#.to_string(),
            r#"[{"op":"nope"}]"#.to_string(),
        ] {
            let e = batch(&ops).unwrap_err();
            assert_eq!(e.code, ErrorCode::InputSchema, "{ops}");
            assert_eq!(e.operation_index, None);
        }
    }

    #[test]
    fn rejects_bad_envelopes() {
        let ops = format!("[{ADD}]");
        for text in [
            format!("{{{BASE},\"operations\":{ops},\"atomic\":false}}")
                .replace("\"atomic\":true,", ""),
            format!("{{{},\"operations\":{ops}}}", BASE.replace("true", "false")),
            format!(
                "{{{},\"operations\":{ops}}}",
                BASE.replace("\"0\"", "\"00\"")
            ),
            format!(
                "{{{},\"operations\":{ops}}}",
                BASE.replace("\"0\"", "99999999999999999999")
            ),
            format!(
                "{{{},\"operations\":{ops}}}",
                BASE.replace("default-v1", "Default")
            ),
            format!(
                "{{{},\"operations\":{ops}}}",
                BASE.replace("1111-4111-8111-111111111111", "1111-4111-8111-11111111111g")
            ),
            format!("{{{},\"operations\":{ops},\"seed\":-1}}", BASE),
        ] {
            assert_eq!(
                Batch::from_json(text.as_bytes()).unwrap_err().code,
                ErrorCode::InputSchema,
                "{text}"
            );
        }
        // A u64-sized 20-digit epoch is syntactically fine but must not overflow.
        let over = format!(
            "{{{},\"operations\":{ops}}}",
            BASE.replace("\"0\"", "\"18446744073709551616\"")
        );
        assert_eq!(
            Batch::from_json(over.as_bytes()).unwrap_err().code,
            ErrorCode::InputSchema
        );
        let max = format!(
            "{{{},\"operations\":{ops}}}",
            BASE.replace("\"0\"", "\"18446744073709551615\"")
        );
        assert_eq!(
            Batch::from_json(max.as_bytes()).unwrap().base_epoch,
            u64::MAX
        );
    }

    #[test]
    fn unknown_but_well_formed_policy_is_left_for_execution() {
        let text = format!(
            "{{{},\"operations\":[{ADD}]}}",
            BASE.replace("default-v1", "missing-v1")
        );
        assert_eq!(
            Batch::from_json(text.as_bytes()).unwrap().policy,
            "paintcraft.core/missing-v1"
        );
    }
}
