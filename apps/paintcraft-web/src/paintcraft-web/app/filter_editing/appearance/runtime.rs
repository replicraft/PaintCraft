//! Exact ready grammar from the real Worker getter, not a manufactured ABI quote.
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value};
use paintcraft_formats::export::layer_appearance::{Admission7, CoverageAbi};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app::filter_editing) struct Runtime7 {
    pub control_bytes: u64,
}
fn bad() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Source7 runtime contract changed",
    )
}
fn decimal(value: &Value) -> Result<u64> {
    let text = value.as_str().ok_or_else(bad)?;
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(bad());
    }
    text.parse().map_err(|_| bad())
}
impl Runtime7 {
    pub fn parse(ready: &Value, noise: bool) -> Result<Self> {
        let Value::Object(wrapper) = ready else {
            return Err(bad());
        };
        if wrapper.len() != 2 {
            return Err(bad());
        }
        let facts = wrapper.get("runtimeFacts").ok_or_else(bad)?;
        let control = wrapper.get("controlQuote").ok_or_else(bad)?;
        let Value::Object(fields) = facts else {
            return Err(bad());
        };
        let quote = fields.get("controlQuoteBytes").ok_or_else(bad)?;
        if control != quote {
            return Err(bad());
        }
        let control_bytes = decimal(quote)?;
        if control_bytes == 0 || control_bytes > Admission7::WORKSPACE_MAX {
            return Err(bad());
        }
        let expected = Value::object([
            ("protocol", Value::Int(7)),
            ("registryContract", Value::Int(3)),
            ("appearanceRevision", Value::Int(1)),
            ("queryRevision", Value::Int(1)),
            ("coverageAbi", Value::str(CoverageAbi::current()?.id())),
            ("sliceRevision", Value::Int(1)),
            ("nativeEdge", Value::Int(64)),
            (
                "baseSliceWork",
                Value::str(Admission7::BASE_SLICE.to_string()),
            ),
            (
                "noiseSliceWork",
                Value::str(if noise { Admission7::NOISE_SLICE } else { 0 }.to_string()),
            ),
            ("controlQuoteBytes", quote.clone()),
        ]);
        if facts != &expected {
            return Err(bad());
        }
        Ok(Self { control_bytes })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(noise: bool, quote: &str) -> Value {
        Value::object([
            (
                "runtimeFacts",
                Value::object([
                    ("protocol", Value::Int(7)),
                    ("registryContract", Value::Int(3)),
                    ("appearanceRevision", Value::Int(1)),
                    ("queryRevision", Value::Int(1)),
                    (
                        "coverageAbi",
                        Value::str(CoverageAbi::current().unwrap().id()),
                    ),
                    ("sliceRevision", Value::Int(1)),
                    ("nativeEdge", Value::Int(64)),
                    ("baseSliceWork", Value::str("8388608")),
                    (
                        "noiseSliceWork",
                        Value::str(if noise { "134217728" } else { "0" }),
                    ),
                    ("controlQuoteBytes", Value::str(quote)),
                ]),
            ),
            ("controlQuote", Value::str(quote)),
        ])
    }
    #[test]
    fn ready_grammar_matches_current_abi_and_exact_fixed_mode_without_quote_authority_claim() {
        // Synthetic parser controls.4096 is not a measured runtime/layout quote.
        assert_eq!(
            Runtime7::parse(&fixture(false, "4096"), false)
                .unwrap()
                .control_bytes,
            4096
        );
        assert!(Runtime7::parse(&fixture(true, "4096"), true).is_ok());
        assert!(Runtime7::parse(&fixture(true, "4096"), false).is_err());
        for quote in ["", "04096", "+4096", "0", "67108865"] {
            assert!(Runtime7::parse(&fixture(false, quote), false).is_err());
        }
        let mut mismatch = fixture(false, "4096");
        let Value::Object(fields) = &mut mismatch else {
            unreachable!()
        };
        fields.insert("controlQuote".into(), Value::str("4097"));
        assert!(Runtime7::parse(&mismatch, false).is_err());
        let mut extra = fixture(false, "4096");
        let Value::Object(fields) = &mut extra else {
            unreachable!()
        };
        fields.insert("extra".into(), Value::Bool(true));
        assert!(Runtime7::parse(&extra, false).is_err());
    }
}
