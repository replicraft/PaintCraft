//! The same explicit MSP version spellings in CLI flags and MCP schemas.
use super::{Error, ErrorCode, ImageFormat, Result};
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    V1,
    V2,
}
impl Version {
    pub fn backend(self) -> paintcraft_formats::msp::Version {
        match self {
            Self::V1 => paintcraft_formats::msp::Version::UncompressedV1,
            Self::V2 => paintcraft_formats::msp::Version::RowRleV2,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }
    pub fn require_format(self, format: ImageFormat) -> Result<()> {
        if format != ImageFormat::Msp {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "MSP version requires MSP output",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_mcp_schema_spellings_match_and_unknown_versions_are_refused() {
        let schema = serde_json::to_value(schemars::schema_for!(Version)).unwrap();
        assert_eq!(schema["enum"], serde_json::json!(["v1", "v2"]));
        for (text, expected) in [("v1", Version::V1), ("v2", Version::V2)] {
            let parsed: Version = serde_json::from_value(serde_json::json!(text)).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.id(), text);
            assert!(parsed.require_format(ImageFormat::Msp).is_ok());
            assert!(parsed.require_format(ImageFormat::Png).is_err());
        }
        for text in ["V1", "v3", "raw", ""] {
            assert!(serde_json::from_value::<Version>(serde_json::json!(text)).is_err());
        }
    }
}
