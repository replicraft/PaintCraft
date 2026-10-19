//! Typed batch v1 (`paintcraft.batch/v1`). Parsing enforces the schema exactly:
//! unknown fields, wrong types, out-of-range numbers and malformed patterns are
//! all `input.schema`. Semantic checks (aliases, targets) happen at execution.

use std::collections::BTreeMap;

use paintcraft_core::types::json::{self, Value};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use paintcraft_core::types::ids::parse_uuid;
use paintcraft_core::types::policy::{COORD_ABS_MAX, hex};

mod stroke;

const SIZE_MAX: i64 = 1_048_576;
const MAX_OPERATIONS: usize = 1024;
const MAX_POINTS: usize = 65_536;
pub const BRUSH_SIZE_MAX: i64 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerRef {
    Alias(String),
    Id(Uuid),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    /// Anchor position in halves: 0, 1 (middle) or 2.
    pub fn halves(self) -> (i64, i64) {
        match self {
            Anchor::TopLeft => (0, 0),
            Anchor::Top => (1, 0),
            Anchor::TopRight => (2, 0),
            Anchor::Left => (0, 1),
            Anchor::Center => (1, 1),
            Anchor::Right => (2, 1),
            Anchor::BottomLeft => (0, 2),
            Anchor::Bottom => (1, 2),
            Anchor::BottomRight => (2, 2),
        }
    }
}

pub use paintcraft_core::types::policy::BlendMode as Mode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
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
    /// v2: a brush stroke of `size` pixels along a path, blended once per pixel.
    StrokePath {
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
    /// v2: text in the built-in bitmap font, laid out from its top-left corner.
    Text {
        layer: LayerRef,
        at: (i64, i64),
        text: String,
        scale: u32,
        color: Rgba,
        mode: Mode,
    },
    CanvasResize {
        width: u32,
        height: u32,
        anchor: Anchor,
    },
}

#[derive(Clone, Debug)]
pub struct Batch {
    /// 1 or 2; version 2 adds the brush, fill and shape operations.
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
    pub fn from_json(bytes: &[u8]) -> Result<Batch> {
        let value = json::parse(bytes).map_err(|e| bad(format!("invalid JSON: {e}")))?;
        Batch::from_value(&value)
    }

    pub fn from_value(v: &Value) -> Result<Batch> {
        let top = obj(
            v,
            &["schema", "atomic", "policy", "base", "operations"],
            &["seed"],
        )?;
        let version = match top["schema"].as_str() {
            Some("paintcraft.batch/v1") => 1,
            Some("paintcraft.batch/v2") => 2,
            _ => {
                return Err(bad(
                    "schema must be paintcraft.batch/v1 or paintcraft.batch/v2",
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
        let operations = ops
            .iter()
            .map(|o| op(o, version))
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

fn op(v: &Value, version: u8) -> Result<Op> {
    let name = v
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("operation needs an op name"))?;
    if version < 2
        && matches!(
            name,
            "raster.stroke_path"
                | "raster.flood_fill"
                | "raster.shape"
                | "raster.text"
                | "layer.reorder"
                | "layer.duplicate"
        )
    {
        return Err(bad(format!(
            "operation {name:?} requires paintcraft.batch/v2"
        )));
    }
    Ok(match name {
        "raster.stroke_path" => stroke::parse(v)?,
        "raster.flood_fill" => {
            let o = obj(
                v,
                &["op", "layer_ref", "at", "color", "mode", "tolerance"],
                &[],
            )?;
            Op::FloodFill {
                layer: layer_ref(&o["layer_ref"])?,
                at: point(&o["at"])?,
                color: color(&o["color"])?,
                mode: mode(&o["mode"])?,
                tolerance: int_in(&o["tolerance"], 0, 255, "tolerance")? as u8,
            }
        }
        "raster.shape" => {
            let o = obj(
                v,
                &["op", "layer_ref", "kind", "from", "to", "mode"],
                &["stroke", "fill"],
            )?;
            let stroke = o
                .get("stroke")
                .map(|s| {
                    let s = obj(s, &["color", "size"], &[])?;
                    Ok((
                        color(&s["color"])?,
                        int_in(&s["size"], 1, BRUSH_SIZE_MAX, "stroke size")? as u32,
                    ))
                })
                .transpose()?;
            let fill = o.get("fill").map(color).transpose()?;
            if stroke.is_none() && fill.is_none() {
                return Err(bad("a shape needs a stroke, a fill or both"));
            }
            if o["kind"].as_str() == Some("line") && stroke.is_none() {
                return Err(bad("a line needs a stroke"));
            }
            Op::Shape {
                layer: layer_ref(&o["layer_ref"])?,
                kind: match o["kind"].as_str() {
                    Some("line") => ShapeKind::Line,
                    Some("rect") => ShapeKind::Rect,
                    Some("ellipse") => ShapeKind::Ellipse,
                    _ => return Err(bad("kind must be line, rect or ellipse")),
                },
                from: point(&o["from"])?,
                to: point(&o["to"])?,
                stroke,
                fill,
                mode: mode(&o["mode"])?,
            }
        }
        "layer.reorder" => {
            let o = obj(v, &["op", "layer_ref", "to_index"], &[])?;
            Op::LayerReorder {
                layer: layer_ref(&o["layer_ref"])?,
                to_index: int_in(&o["to_index"], 0, 99_999, "to_index")? as usize,
            }
        }
        "raster.text" => {
            let o = obj(
                v,
                &["op", "layer_ref", "at", "text", "scale", "color", "mode"],
                &[],
            )?;
            let text = string(&o["text"], "text")?;
            if text.is_empty()
                || text.chars().count() > paintcraft_core::types::policy::TEXT_CHARS_MAX
            {
                return Err(bad("text must contain 1..=1024 characters"));
            }
            Op::Text {
                layer: layer_ref(&o["layer_ref"])?,
                at: point(&o["at"])?,
                text,
                scale: int_in(
                    &o["scale"],
                    1,
                    paintcraft_core::types::policy::SCALE_MAX,
                    "scale",
                )? as u32,
                color: color(&o["color"])?,
                mode: mode(&o["mode"])?,
            }
        }
        "layer.add" => {
            let o = obj(
                v,
                &[
                    "op",
                    "ref",
                    "name",
                    "width",
                    "height",
                    "origin",
                    "default_pixel",
                ],
                &[],
            )?;
            Op::LayerAdd {
                alias: alias(&o["ref"])?,
                name: name_str(&o["name"])?,
                width: int_in(&o["width"], 1, SIZE_MAX, "width")? as u32,
                height: int_in(&o["height"], 1, SIZE_MAX, "height")? as u32,
                origin: point(&o["origin"])?,
                default: color(&o["default_pixel"])?,
            }
        }
        "layer.duplicate" => {
            let o = obj(v, &["op", "layer_ref", "ref", "name"], &[])?;
            Op::LayerDuplicate {
                layer: layer_ref(&o["layer_ref"])?,
                alias: alias(&o["ref"])?,
                name: name_str(&o["name"])?,
            }
        }
        "layer.delete" => {
            let o = obj(v, &["op", "layer_ref"], &[])?;
            Op::LayerDelete {
                layer: layer_ref(&o["layer_ref"])?,
            }
        }
        "layer.set_properties" => {
            let o = obj(v, &["op", "layer_ref"], &["name", "visible", "opacity_u8"])?;
            Op::LayerSetProperties {
                layer: layer_ref(&o["layer_ref"])?,
                name: o.get("name").map(name_str).transpose()?,
                visible: o
                    .get("visible")
                    .map(|b| b.as_bool().ok_or_else(|| bad("visible must be boolean")))
                    .transpose()?,
                opacity: o
                    .get("opacity_u8")
                    .map(|n| int_in(n, 0, 255, "opacity_u8"))
                    .transpose()?
                    .map(|n| n as u8),
            }
        }
        "raster.fill_rect" => {
            let o = obj(v, &["op", "layer_ref", "rect", "color", "mode"], &[])?;
            Op::FillRect {
                layer: layer_ref(&o["layer_ref"])?,
                rect: rect(&o["rect"])?,
                color: color(&o["color"])?,
                mode: mode(&o["mode"])?,
            }
        }
        "raster.pencil_path" => {
            let o = obj(v, &["op", "layer_ref", "points", "color", "mode"], &[])?;
            let pts = o["points"]
                .as_array()
                .ok_or_else(|| bad("points must be an array"))?;
            if pts.is_empty() || pts.len() > MAX_POINTS {
                return Err(bad("points must contain 1..=65536 items"));
            }
            Op::PencilPath {
                layer: layer_ref(&o["layer_ref"])?,
                points: pts.iter().map(point).collect::<Result<_>>()?,
                color: color(&o["color"])?,
                mode: mode(&o["mode"])?,
            }
        }
        "selection.set_rect" => {
            let o = obj(v, &["op", "rect"], &[])?;
            Op::SelectionSetRect {
                rect: rect(&o["rect"])?,
            }
        }
        "selection.clear" => {
            obj(v, &["op"], &[])?;
            Op::SelectionClear
        }
        "canvas.resize" => {
            let o = obj(
                v,
                &["op", "width", "height", "anchor", "outside_content"],
                &[],
            )?;
            if o["outside_content"].as_str() != Some("preserve") {
                return Err(bad("outside_content must be preserve"));
            }
            Op::CanvasResize {
                width: int_in(&o["width"], 1, SIZE_MAX, "width")? as u32,
                height: int_in(&o["height"], 1, SIZE_MAX, "height")? as u32,
                anchor: match o["anchor"].as_str() {
                    Some("top-left") => Anchor::TopLeft,
                    Some("top") => Anchor::Top,
                    Some("top-right") => Anchor::TopRight,
                    Some("left") => Anchor::Left,
                    Some("center") => Anchor::Center,
                    Some("right") => Anchor::Right,
                    Some("bottom-left") => Anchor::BottomLeft,
                    Some("bottom") => Anchor::Bottom,
                    Some("bottom-right") => Anchor::BottomRight,
                    _ => return Err(bad("unknown anchor")),
                },
            }
        }
        other => return Err(bad(format!("unknown operation {other:?}"))),
    })
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
