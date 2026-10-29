//! Explicit multi-size icon exports, separate from ordinary flat-image export.
use super::super::{Context, PathBuf, same_file, save};
use super::{
    PaintingServer,
    state::{core_json, invalid},
};
use paintcraft_formats::{
    icons,
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
#[serde(rename_all = "kebab-case")]
enum Encoding {
    Png,
    Bitmap,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Resampling {
    Nearest,
    #[default]
    Lanczos3,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Entry {
    width: u16,
    height: u16,
    encoding: Encoding,
    /// Required for CUR, forbidden for ICO. Zero-based [x,y] inside this entry.
    hotspot: Option<[u16; 2]>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Export {
    /// Explicitly omit original source metadata from this copy; default refuse.
    source_metadata: Option<super::params::SourceMetadata>,
    /// A fresh .ico or .cur destination; never a foreign preserving save.
    path: String,
    /// Ordered entries, each dimension 1..256. Every size is fitted independently.
    entries: Vec<Entry>,
    /// Fit the source aspect ratio onto transparent padding; defaults to lanczos3.
    #[serde(default)]
    resampling: Resampling,
    #[serde(default)]
    overwrite: bool,
    /// Acknowledge actual losses: flatten, hidden-rgb, icon-resized.
    #[serde(default)]
    allow_losses: Vec<String>,
}

#[tool_router(router = icon_router, vis = "pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Export fresh multi-size ICO/CUR assets with explicit PNG/bitmap entries and cursor hotspots. Fits each size from the original canvas. Requires allow_losses consent, including icon-resized when sizes differ. Preserves the open project and dirty state."
    )]
    async fn export_icons(&self, Parameters(p): Parameters<Export>) -> CallToolResult {
        self.work(move |state| {
            icons::check_export_loss_codes(&p.allow_losses)?;
            let path = PathBuf::from(p.path);
            state.protect_copy_sources(&path)?;
            if state
                .source
                .as_ref()
                .is_some_and(|(source, _)| same_file(source, &path))
            {
                return Err(invalid("an exported icon must not replace its source"));
            }
            let family = path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Family::from_extension)
                .filter(|family| matches!(family, Family::Ico | Family::Cur))
                .ok_or_else(|| invalid("icon output requires an .ico or .cur extension"))?;
            let mut request = Request::new(
                Frontend::Studio,
                Operation::SaveAsCopy,
                family,
                Platform::Native,
            );
            request.extension = path.extension().and_then(|ext| ext.to_str());
            request.variant = Some("icon-fresh-entry-set");
            let decision = registry::admission(request);
            if !decision.available() {
                decision.require_allowed(request)?;
            }
            save::preflight(&path, p.overwrite)?;
            let specs: Vec<_> = p
                .entries
                .into_iter()
                .map(|entry| icons::EntrySpec {
                    width: u32::from(entry.width),
                    height: u32::from(entry.height),
                    encoding: match entry.encoding {
                        Encoding::Png => icons::Encoding::Png,
                        Encoding::Bitmap => icons::Encoding::Bitmap,
                    },
                    hotspot: entry.hotspot.map(|[x, y]| (x, y)),
                })
                .collect();
            let kind = if family == Family::Ico {
                icons::Kind::Icon
            } else {
                icons::Kind::Cursor
            };
            let filter = match p.resampling {
                Resampling::Nearest => icons::Filter::Nearest,
                Resampling::Lanczos3 => icons::Filter::Lanczos3,
            };
            let doc = state.session()?.document();
            let policy = super::super::export_metadata::CopyFormat::Icon(kind)
                .capture(doc, p.source_metadata.unwrap_or_default().into())?;
            let prepared = icons::prepare_document_with_metadata_policy(
                doc,
                kind,
                &specs,
                filter,
                icons::WriteLimits::default(),
                &|| Ok(()),
                policy,
            )?;
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
