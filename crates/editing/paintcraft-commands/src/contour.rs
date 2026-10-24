//! Bounded polygon/polyline command grammar; curves arrive as flattened paths.
use super::*;

pub(super) fn parse(v: &Value) -> Result<Op> {
    let o = obj(
        v,
        &["op", "layer_ref", "points", "closed", "mode"],
        &["stroke", "fill"],
    )?;
    let closed = o["closed"]
        .as_bool()
        .ok_or_else(|| bad("closed must be boolean"))?;
    let values = o["points"]
        .as_array()
        .ok_or_else(|| bad("points must be an array"))?;
    if values.len() < if closed { 3 } else { 2 } || values.len() > 1024 {
        return Err(bad(
            "closed contours need 3..=1024 points; open contours need 2..=1024",
        ));
    }
    let points = values.iter().map(point).collect::<Result<Vec<_>>>()?;
    let stroke = o
        .get("stroke")
        .map(|v| {
            let s = obj(v, &["color", "size"], &[])?;
            Ok((
                color(&s["color"])?,
                int_in(&s["size"], 1, BRUSH_SIZE_MAX, "stroke size")? as u32,
            ))
        })
        .transpose()?;
    let fill = o.get("fill").map(color).transpose()?;
    if stroke.is_none() && fill.is_none() {
        return Err(bad("a contour needs a stroke or fill"));
    }
    if !closed && (stroke.is_none() || fill.is_some()) {
        return Err(bad("open contours require a stroke and cannot be filled"));
    }
    Ok(Op::Contour {
        layer: layer_ref(&o["layer_ref"])?,
        points,
        closed,
        stroke,
        fill,
        mode: mode(&o["mode"])?,
    })
}
