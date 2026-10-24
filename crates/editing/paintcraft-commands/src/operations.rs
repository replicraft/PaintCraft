//! Versioned operation grammar.
use super::*;
pub(super) fn op(v: &Value, version: u8) -> Result<Op> {
    let name = v
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("operation needs an op name"))?;
    if version < 2
        && matches!(
            name,
            "canvas.clear"
                | "layer.expand"
                | "raster.stroke_path"
                | "raster.flood_fill"
                | "raster.shape"
                | "raster.contour"
                | "canvas.crop"
                | "canvas.transform"
                | "raster.transform_selection"
                | "raster.text"
                | "raster.mask_spans"
                | "layer.reorder"
                | "layer.duplicate"
        )
    {
        return Err(bad(format!(
            "operation {name:?} requires paintcraft.batch/v2"
        )));
    }
    Ok(match name {
        "raster.transform_selection" => {
            let o = obj(v, &["op", "layer_ref", "transform", "clear"], &[])?;
            Op::TransformSelection {
                layer: layer_ref(&o["layer_ref"])?,
                transform: o["transform"]
                    .as_str()
                    .and_then(paintcraft_core::types::orthogonal::OrthogonalTransform::parse)
                    .ok_or_else(|| bad("unknown orthogonal transform"))?,
                clear: color(&o["clear"])?,
            }
        }

        "canvas.transform" => {
            let o = obj(v, &["op", "transform"], &[])?;
            Op::CanvasTransform {
                transform: o["transform"]
                    .as_str()
                    .and_then(paintcraft_core::types::orthogonal::OrthogonalTransform::parse)
                    .ok_or_else(|| bad("unknown orthogonal transform"))?,
            }
        }

        "raster.stroke_path" => stroke::parse(v)?,
        "raster.contour" => contour::parse(v)?,
        "canvas.crop" => {
            let o = obj(v, &["op", "rect"], &[])?;
            Op::CanvasCrop {
                rect: rect(&o["rect"])?,
            }
        }
        "raster.mask_spans" => mask::parse(v)?,
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
                    Some(name) => ShapeKind::Preset(
                        paintcraft_core::types::shapes::ShapePreset::from_command_name(name)
                            .ok_or_else(|| bad("unknown shape kind"))?,
                    ),
                    _ => return Err(bad("shape kind must be a string")),
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
        "layer.expand" => {
            let o = obj(v, &["op", "layer_ref", "width", "height"], &[])?;
            Op::LayerExpand {
                layer: layer_ref(&o["layer_ref"])?,
                width: int_in(&o["width"], 1, SIZE_MAX, "width")? as u32,
                height: int_in(&o["height"], 1, SIZE_MAX, "height")? as u32,
            }
        }
        "canvas.clear" => {
            let o = obj(v, &["op", "ref", "color"], &[])?;
            Op::CanvasClear {
                alias: alias(&o["ref"])?,
                color: color(&o["color"])?,
            }
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
