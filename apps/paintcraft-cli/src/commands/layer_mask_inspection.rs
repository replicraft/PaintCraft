//! Finite immutable metadata only; inspection never expands A8 pixels or runs.
use paintcraft_core::types::{json::Value, policy::hex};
use paintcraft_document::LayerMask;
pub(super) fn value(mask: &LayerMask) -> Value {
    let domain = mask.coverage().domain();
    let stats = mask.coverage().stats();
    Value::object([
        ("schema", Value::str("a8-bands/v1")),
        ("enabled", Value::Bool(mask.enabled())),
        (
            "domain",
            Value::Array(
                [domain.x0, domain.y0, domain.x1, domain.y1]
                    .into_iter()
                    .map(Value::Int)
                    .collect(),
            ),
        ),
        ("bands", Value::Int(stats.bands as i64)),
        ("runs", Value::Int(stats.runs as i64)),
        ("digest", Value::str(hex(&mask.coverage().digest()))),
        ("intent_digest", Value::str(hex(&mask.digest()))),
    ])
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{Rect, resources::SharedBudget};
    use paintcraft_raster::coverage_mask::A8Mask;
    #[test]
    fn finite_metadata_keeps_enabled_domain_and_both_distinct_digest_meanings() {
        let budget = SharedBudget::new(8192);
        let coverage = A8Mask::uniform(
            Rect::new(-1, -1, 4, 3).unwrap(),
            127,
            &budget,
            Default::default(),
            &|| Ok(()),
        )
        .unwrap();
        let mask = LayerMask::new(false, coverage);
        let v = value(&mask);
        assert_eq!(
            v.get("digest").and_then(Value::as_str),
            Some("5e38cf1164b88cd9c781811765f578f0ecbb54d0cf24c32a0f5d7b9dabf9a549")
        );
        assert_eq!(
            v.get("intent_digest").and_then(Value::as_str),
            Some("42d8ee64eddf83b947e75e026f5eda4fc1444bb5979db1ed996abdeb2b98b5d7")
        );
        assert_eq!(
            v.get("domain"),
            Some(&Value::Array(vec![
                Value::Int(-1),
                Value::Int(-1),
                Value::Int(3),
                Value::Int(2)
            ]))
        );
        let changed = value(&mask.with_enabled(true));
        assert_eq!(v.get("digest"), changed.get("digest"));
        assert_ne!(v.get("intent_digest"), changed.get("intent_digest"));
        let Value::Object(fields) = v else {
            panic!("object")
        };
        assert_eq!(fields.len(), 7);
        assert!(!fields.contains_key("pixels"));
        assert!(!fields.contains_key("bands_payload"));
    }
}
