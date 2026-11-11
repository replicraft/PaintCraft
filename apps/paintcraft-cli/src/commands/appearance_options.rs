//! Raw user intent, independent from legacy allow-loss filter authority.
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum AppearanceCopy {
    #[default]
    Refuse,
    Bake,
}
impl From<AppearanceCopy> for paintcraft_formats::export::appearance_bake::Intent {
    fn from(value: AppearanceCopy) -> Self {
        match value {
            AppearanceCopy::Refuse => Self::Refuse,
            AppearanceCopy::Bake => Self::Bake,
        }
    }
}
