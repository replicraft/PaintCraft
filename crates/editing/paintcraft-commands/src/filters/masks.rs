//! Strict v3 numeric mask operations; no Raster or Document dependencies.
use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{A8Input, A8InputBuilder, A8Run, MaskInputLimits},
    resources::SharedBudget,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaskOp {
    Set { coverage: A8Input, enabled: bool },
    FromSelection { enabled: bool },
    SetEnabled { enabled: bool },
    Remove,
}
fn array<'a>(v: &'a Value, label: &str) -> Result<&'a [Value]> {
    v.as_array()
        .ok_or_else(|| bad(format!("{label} must be an array")))
}
fn coordinate(v: &Value) -> Result<i64> {
    int_in(v, -COORD_ABS_MAX, COORD_ABS_MAX, "mask coordinate")
}
fn decode(v: &Value, budget: &SharedBudget, check: &dyn Fn() -> Result<()>) -> Result<A8Input> {
    let mask = obj(v, &["domain", "bands"], &[])?;
    let d = obj(&mask["domain"], &["x", "y", "width", "height"], &[])?;
    let domain = Rect::new(
        coordinate(&d["x"])?,
        coordinate(&d["y"])?,
        int_in(&d["width"], 1, 2 * COORD_ABS_MAX, "mask width")?,
        int_in(&d["height"], 1, 2 * COORD_ABS_MAX, "mask height")?,
    )
    .ok_or_else(|| bad("invalid mask domain"))?;
    let bands = array(&mask["bands"], "mask bands")?;
    let limits = MaskInputLimits::default();
    if bands.len() > limits.bands {
        return Err(Error::new(ErrorCode::ResourceLimit, "Mask band count"));
    }
    let mut count = 0usize;
    for band in bands {
        check()?;
        let o = obj(band, &["y0", "y1", "runs"], &[])?;
        count = count
            .checked_add(array(&o["runs"], "mask runs")?.len())
            .filter(|n| *n <= limits.runs)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "Mask run count"))?;
    }
    let mut builder = A8InputBuilder::new(domain, bands.len(), count, budget, limits, check)?;
    for band in bands {
        let o = band.as_object().expect("validated band");
        builder.push_band(coordinate(&o["y0"])?, coordinate(&o["y1"])?)?;
        for run in array(&o["runs"], "mask runs")? {
            let r = obj(run, &["x0", "x1", "coverage"], &[])?;
            builder.push_run(A8Run {
                x0: coordinate(&r["x0"])?,
                x1: coordinate(&r["x1"])?,
                coverage: int_in(&r["coverage"], 0, 255, "A8 coverage")? as u8,
            })?;
        }
    }
    builder.finish()
}
pub(super) fn parse(
    v: &Value,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Op> {
    let name = v
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("mask operation needs op"))?;
    let (required, optional): (&[&str], &[&str]) = match name {
        "filter.mask.set" => (&["op", "layer_ref", "filter_ref", "mask"], &["enabled"]),
        "filter.mask.from_selection" => (&["op", "layer_ref", "filter_ref"], &["enabled"]),
        "filter.mask.set_enabled" => (&["op", "layer_ref", "filter_ref", "enabled"], &[]),
        "filter.mask.remove" => (&["op", "layer_ref", "filter_ref"], &[]),
        _ => return Err(bad("Unknown filter mask operation")),
    };
    let o = obj(v, required, optional)?;
    let layer = layer_ref(&o["layer_ref"])?;
    let filter = reference(&o["filter_ref"])?;
    let enabled = || {
        o.get("enabled")
            .map(boolean)
            .transpose()
            .map(|v| v.unwrap_or(true))
    };
    let operation = match name {
        "filter.mask.set" => MaskOp::Set {
            coverage: decode(&o["mask"], budget, check)?,
            enabled: enabled()?,
        },
        "filter.mask.from_selection" => MaskOp::FromSelection {
            enabled: enabled()?,
        },
        "filter.mask.set_enabled" => MaskOp::SetEnabled {
            enabled: enabled()?,
        },
        "filter.mask.remove" => MaskOp::Remove,
        _ => unreachable!(),
    };
    Ok(Op::Filter(FilterOp::Mask {
        layer,
        filter,
        operation,
    }))
}
fn encode(mask: &A8Input) -> Value {
    let d = mask.domain();
    Value::object([
        (
            "domain",
            Value::object([
                ("x", Value::Int(d.x0)),
                ("y", Value::Int(d.y0)),
                ("width", Value::Int(d.width())),
                ("height", Value::Int(d.height())),
            ]),
        ),
        (
            "bands",
            Value::Array(
                mask.bands()
                    .map(|b| {
                        Value::object([
                            ("y0", Value::Int(b.y0)),
                            ("y1", Value::Int(b.y1)),
                            (
                                "runs",
                                Value::Array(
                                    b.runs
                                        .iter()
                                        .map(|r| {
                                            Value::object([
                                                ("x0", Value::Int(r.x0)),
                                                ("x1", Value::Int(r.x1)),
                                                ("coverage", Value::Int(i64::from(r.coverage))),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}
pub(super) fn to_value(op: &FilterOp) -> Result<Value> {
    let FilterOp::Mask {
        layer,
        filter,
        operation,
    } = op
    else {
        return Err(bad("Expected filter mask operation"));
    };
    let layer = match layer {
        LayerRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
        LayerRef::Alias(a) => Value::object([("ref", Value::str(a))]),
    };
    let filter = match filter {
        FilterRef::Id(id) => Value::object([("id", Value::str(id.to_string()))]),
        FilterRef::Alias(a) => Value::object([("ref", Value::str(a))]),
    };
    layer_ref(&layer)?;
    reference(&filter)?;
    let (name, extra) = match operation {
        MaskOp::Set { coverage, enabled } => (
            "filter.mask.set",
            vec![
                ("mask", encode(coverage)),
                ("enabled", Value::Bool(*enabled)),
            ],
        ),
        MaskOp::FromSelection { enabled } => (
            "filter.mask.from_selection",
            vec![("enabled", Value::Bool(*enabled))],
        ),
        MaskOp::SetEnabled { enabled } => (
            "filter.mask.set_enabled",
            vec![("enabled", Value::Bool(*enabled))],
        ),
        MaskOp::Remove => ("filter.mask.remove", vec![]),
    };
    let Value::Object(mut o) = Value::object([
        ("op", Value::str(name)),
        ("layer_ref", layer),
        ("filter_ref", filter),
    ]) else {
        unreachable!()
    };
    for (k, v) in extra {
        o.insert(k.into(), v);
    }
    Ok(Value::Object(o))
}

#[cfg(test)]
mod tests;
