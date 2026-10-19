use super::*;

pub(super) fn parse(v: &Value) -> Result<Op> {
    let o = obj(
        v,
        &[
            "op",
            "layer_ref",
            "points",
            "color",
            "mode",
            "size",
            "brush",
        ],
        &["point_scale"],
    )?;
    let smooth = o["brush"].as_str() == Some("smooth-round");
    let point_scale = match o.get("point_scale") {
        None => 1,
        Some(Value::Int(1)) => 1,
        Some(Value::Int(256)) if smooth => 256,
        _ => return Err(bad("point_scale must be 1, or 256 for smooth-round")),
    };
    let pts = o["points"]
        .as_array()
        .ok_or_else(|| bad("points must be an array"))?;
    if pts.is_empty() || pts.len() > MAX_POINTS {
        return Err(bad("points must contain 1..=65536 items"));
    }
    let bound = COORD_ABS_MAX * i64::from(point_scale);
    let points = pts
        .iter()
        .map(|v| {
            let p = obj(v, &["x", "y"], &[])?;
            Ok((
                int_in(&p["x"], -bound, bound, "x")?,
                int_in(&p["y"], -bound, bound, "y")?,
            ))
        })
        .collect::<Result<_>>()?;
    Ok(Op::StrokePath {
        layer: layer_ref(&o["layer_ref"])?,
        points,
        color: color(&o["color"])?,
        mode: mode(&o["mode"])?,
        size: int_in(&o["size"], 1, BRUSH_SIZE_MAX, "size")? as u32,
        round: match o["brush"].as_str() {
            Some("round" | "smooth-round") => true,
            Some("square") => false,
            _ => return Err(bad("brush must be round, square or smooth-round")),
        },
        smooth,
        point_scale,
    })
}
