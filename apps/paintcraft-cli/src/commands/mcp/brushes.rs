//! Explicit native-static brush resources, independent of document creation.
use super::super::{PathBuf, brushes};
use super::{
    PaintingServer,
    state::{core_json, invalid, reply},
};
use paintcraft_core::types::{Error, ErrorCode, resources::SharedBudget};
use paintcraft_formats::{abr::adoption::AdoptedPreset, registry::Frontend};
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
pub(super) struct Selected {
    pub loaded: Arc<brushes::Loaded>,
    pub preset: AdoptedPreset,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspect {
    path: String,
    expect_input_sha256: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Adopt {
    path: String,
    preset: usize,
    /// Must be native-grid-v1: immutable native R8 pixels, max-union coverage.
    policy: String,
    #[serde(default)]
    allow_spacing_quantization: bool,
    expect_input_sha256: Option<String>,
}
#[tool_router(router=brush_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Inspect an ABR brush asset without opening or modifying a painting. Reports ordered presets, qualified native-static R8 candidates, unsupported behavior and required conversions. Exact source samples/descriptors remain retained by adoption; no Photoshop rendering parity is implied."
    )]
    async fn inspect_brushes(&self, Parameters(p): Parameters<Inspect>) -> CallToolResult {
        self.work(move |_| {
            let loaded = brushes::acquire(
                &PathBuf::from(p.path),
                p.expect_input_sha256.as_deref(),
                Frontend::Studio,
            )?;
            Ok(reply(brushes::inspection(&loaded)?))
        })
        .await
    }
    #[tool(
        description = "Select one ABR preset under explicit native-grid-v1 PaintCraft static-tip policy, without changing document pixels/history/save binding. Preserves source and exact R8 tip; refuses computed/high-depth/transformed/dynamic/textured/unknown presets. Spacing quantization requires explicit acknowledgement. Selection is ephemeral, not saved as a stock brush."
    )]
    async fn adopt_brush(&self, Parameters(p): Parameters<Adopt>) -> CallToolResult {
        self.work(move |state|{
            let loaded=Arc::new(brushes::acquire(&PathBuf::from(p.path),p.expect_input_sha256.as_deref(),Frontend::Studio)?);
            let budget=SharedBudget::new(brushes::BUDGET_BYTES);
            let preset=loaded.adopt(p.preset,&p.policy,p.allow_spacing_quantization,&budget)?;
            let result=json!({"source_sha256":loaded.sha,"preset":preset.preset_index(),"policy":"native-grid-v1","width":preset.tip().width(),"height":preset.tip().height(),"spacing_q8":preset.spacing_q8(),"painted":false});
            state.selected_brush=Some(Selected{loaded,preset});Ok(reply(result))
        }).await
    }
    #[tool(
        description = "Paint with the selected imported native-static R8 tip using captured integer [x,y] points, color #RRGGBBAA and point_scale 1 or 256. Uses the existing atomic mask executor and one Brush Undo entry; opacity is applied once. Unsupported preset behavior is never mapped to a round brush. Empty or zero-opacity coverage creates no document/history edit."
    )]
    async fn paint_sampled_stroke(
        &self,
        Parameters(p): Parameters<brushes::Stroke>,
    ) -> CallToolResult {
        self.work(move |state| {
            let selected = state
                .selected_brush
                .as_ref()
                .ok_or_else(|| invalid("adopt a qualified brush first"))?;
            let loaded = selected.loaded.clone();
            let preset = selected.preset.clone();
            if state.brush_sources.len() >= 16
                && !state
                    .brush_sources
                    .iter()
                    .any(|source| source.same_resource(&loaded))
            {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "painting retains at most sixteen brush sources",
                ));
            }
            let receipt = brushes::paint(
                state.session()?,
                &preset,
                p,
                &SharedBudget::new(brushes::BUDGET_BYTES),
            )?;
            if receipt.committed
                && !state
                    .brush_sources
                    .iter()
                    .any(|source| source.same_resource(&loaded))
            {
                state.brush_sources.push(loaded);
            }
            Ok(reply(core_json(&receipt.to_value())?))
        })
        .await
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn renamed_unreadable_brush_alias_fails_closed_in_the_direct_output_guard() {
        let folder =
            std::env::temp_dir().join(format!("paintcraft-brush-guard-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let source = folder.join("source.abr");
        let moved = folder.join("moved.abr");
        let alias = folder.join("alias.pcnd");
        std::fs::write(&source,include_bytes!("../../../../../crates/services/paintcraft-formats/tests/abr-fixtures/native-presets.abr")).unwrap();
        let loaded = Arc::new(brushes::acquire(&source, None, Frontend::Studio).unwrap());
        let preset = loaded
            .adopt(0, "native-grid-v1", false, &SharedBudget::new(1 << 20))
            .unwrap();
        let state = super::super::state::State {
            selected_brush: Some(Selected { loaded, preset }),
            ..Default::default()
        };
        std::fs::rename(&source, &moved).unwrap();
        std::fs::hard_link(&moved, &alias).unwrap();
        let original = std::fs::metadata(&moved).unwrap().permissions();
        std::fs::set_permissions(&moved, std::fs::Permissions::from_mode(0o0)).unwrap();
        let denied = state.protect_import_sources(&alias);
        let readable = std::fs::File::open(&alias).is_ok();
        std::fs::set_permissions(&moved, original).unwrap();
        // Elevated test processes may bypass mode bits; identity rejection is
        // still required. Ordinary users must propagate the failed open.
        assert_eq!(
            denied.unwrap_err().code,
            if readable {
                ErrorCode::InputSchema
            } else {
                ErrorCode::StorageFailed
            }
        );
        assert!(state.protect_import_sources(&alias).is_err());
        drop(state);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
