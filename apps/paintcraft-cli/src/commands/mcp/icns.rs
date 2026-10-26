//! Explicit typed ICNS exports from immutable fitted preview buffers.
use super::super::{Context, PathBuf, same_file, save};
use super::{
    PaintingServer,
    state::{core_json, invalid},
};
use paintcraft_formats::{
    icns, icons,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
enum Type {
    #[serde(rename = "ic07")]
    Size128,
    #[serde(rename = "ic08")]
    Size256,
    #[serde(rename = "ic09")]
    Size512,
    #[serde(rename = "ic10")]
    Retina512,
    #[serde(rename = "ic11")]
    Retina16,
    #[serde(rename = "ic12")]
    Retina32,
    #[serde(rename = "ic13")]
    Retina128,
    #[serde(rename = "ic14")]
    Retina256,
}
impl From<Type> for icns::IconType {
    fn from(value: Type) -> Self {
        match value {
            Type::Size128 => Self::Size128,
            Type::Size256 => Self::Size256,
            Type::Size512 => Self::Size512,
            Type::Retina512 => Self::Retina512,
            Type::Retina16 => Self::Retina16,
            Type::Retina32 => Self::Retina32,
            Type::Retina128 => Self::Retina128,
            Type::Retina256 => Self::Retina256,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Filter {
    Nearest,
    #[default]
    Lanczos3,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Export {
    /// Fresh .icns destination. Source-preserving saves are unavailable.
    path: String,
    /// Explicit ordered types; ic11 is 16 logical pixels at 2x, ic12 is 32 at 2x.
    /// Qualified PNG types only. No automatic Retina substitution or resize choice.
    types: Vec<Type>,
    /// Fits every entry independently on transparent padding from the original canvas.
    #[serde(default)]
    resampling: Filter,
    #[serde(default)]
    overwrite: bool,
    /// Acknowledge actual flatten, hidden-rgb and icon-resized conversions.
    #[serde(default)]
    allow_losses: Vec<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspect {
    path: String,
    expect_source_sha256: Option<String>,
}
#[tool_router(router=icns_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Inspect ordered ICNS entries and source SHA without decoding pixels or changing the current project. Returns exact tags, logical sizes, density, physical dimensions and import qualification; use the explicit index with import_image entry."
    )]
    async fn inspect_icns(&self, Parameters(p): Parameters<Inspect>) -> CallToolResult {
        self.work(move |_| {
            let (value, _) = super::super::inspect_icns::inspect_path(
                &PathBuf::from(p.path),
                Frontend::Studio,
                p.expect_source_sha256.as_deref(),
            )?;
            Ok(CallToolResult::success(vec![ContentBlock::text(
                core_json(&value)?.to_string(),
            )]))
        })
        .await
    }
    #[tool(
        description = "Export a fresh ICNS set using explicit PNG tags and logical/Retina densities. ic11 is 16@2; ic12 is 32@2. Fits each size independently and requires actual loss consent. Small 1x, legacy RGB/mask, and ARGB writing are refused. Leaves the project and dirty state unchanged."
    )]
    async fn export_icns(&self, Parameters(p): Parameters<Export>) -> CallToolResult {
        self.work(move |state| {
            icons::check_export_loss_codes(&p.allow_losses)?;
            let path = PathBuf::from(p.path);
            state.protect_copy_sources(&path)?;
            if state
                .source
                .as_ref()
                .is_some_and(|(source, _)| same_file(source, &path))
            {
                return Err(invalid("ICNS export must not replace its source project"));
            }
            let mut request = Request::new(
                Frontend::Studio,
                Operation::SaveAsCopy,
                Family::Icns,
                Platform::Native,
            );
            request.extension = path.extension().and_then(|value| value.to_str());
            if request.extension.is_none() {
                return Err(invalid("ICNS output requires .icns extension"));
            }
            request.variant = Some("icon-fresh-entry-set");
            match registry::admission(request) {
                registry::Decision::Allowed
                | registry::Decision::AllowedWithPreflight(
                    registry::Reason::ConvertRequiresConsent,
                ) => {}
                decision => decision.require_allowed(request)?,
            }
            // The only resolved preflight is the exact prepared pixel/loss consent below.
            save::preflight(&path, p.overwrite)?;
            let kinds: Vec<_> = p.types.into_iter().map(Into::into).collect();
            let filter = match p.resampling {
                Filter::Nearest => icons::Filter::Nearest,
                Filter::Lanczos3 => icons::Filter::Lanczos3,
            };
            let doc = state.session()?.document();
            let prepared = icns::prepare_document(doc, &kinds, filter, &|| Ok(()))?;
            let bytes = prepared.encode(&p.allow_losses, &|| Ok(()))?;
            let context = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: None,
                losses_acknowledged: prepared.losses().to_vec(),
                destination: &path,
            };
            let outcome = state.write_copy_with(&path, p.overwrite, |sink| {
                sink.write_all(&bytes)
                    .map_err(|error| super::super::storage("staged icon", &error))
            })?;
            let (receipt, ok, _) = super::super::finish_output(&path, &context, outcome);
            let content = vec![ContentBlock::text(core_json(&receipt)?.to_string())];
            Ok(if ok {
                CallToolResult::success(content)
            } else {
                CallToolResult::error(content)
            })
        })
        .await
    }
}
