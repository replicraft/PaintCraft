//! Serialized digest-bound float inspection/adoption with original-source guards.
use super::super::{PathBuf, Session, codec_worker, float_job, float_options::Selection};
use super::{
    PaintingServer, native_codec,
    state::{State, core_json, invalid, reply},
};
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::registry::Frontend;
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::{Arc, atomic::Ordering};
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspect {
    path: String,
    settings: Selection,
    expect_input_sha256: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Import {
    path: String,
    settings: Selection,
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    expect_input_sha256: String,
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    expect_assessment_sha256: String,
    #[serde(default)]
    allow_losses: Vec<String>,
    #[serde(default)]
    discard_changes: bool,
}
fn cancelled() -> Error {
    Error::new(
        ErrorCode::OperationCancelled,
        "Float conversion request was cancelled",
    )
}
#[tool_router(router=float_router,vis="pub(super)")]
impl PaintingServer {
    #[tool(
        description = "Inspect an HDR/EXR source under explicit float-to-SDR settings without replacing the current painting. Returns raw source and assessment SHA256, actual losses, statistics and metadata. Does not adopt pixels or change saved/dirty state; exact native Studio qualification remains required."
    )]
    async fn inspect_float(
        &self,
        Parameters(p): Parameters<Inspect>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = native_codec::RpcLife::new();
        let dropped = life.0.clone();
        let cancellation = context.ct.clone();
        let check: float_job::Check = Arc::new(move || {
            if dropped.load(Ordering::Acquire) || cancellation.is_cancelled() {
                Err(cancelled())
            } else {
                Ok(())
            }
        });
        self.work(move |state| {
            let captured = native_codec::owner(state, codec_worker::owner());
            let prepared = float_job::prepare(
                &PathBuf::from(p.path),
                p.settings,
                p.expect_input_sha256,
                captured,
                native_codec::preparation_budget(),
                check.clone(),
                Frontend::Studio,
            )?;
            let value = core_json(&prepared.inspection()?)?;
            check()?;
            Ok(reply(value))
        })
        .await
    }
    #[tool(
        description = "Import an explicitly assessed HDR/EXR conversion into a new unsaved painting. Requires exact raw source SHA256, full settings, matching assessment SHA256 and every actual loss. Unsaved edits require discard_changes. Refusal/cancellation preserves the old document/history/save binding; source-preserving Save and generic Open are not enabled."
    )]
    async fn import_float(
        &self,
        Parameters(p): Parameters<Import>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> CallToolResult {
        let life = native_codec::RpcLife::new();
        let dropped = life.0.clone();
        let cancellation = context.ct.clone();
        let check: float_job::Check = Arc::new(move || {
            if dropped.load(Ordering::Acquire) || cancellation.is_cancelled() {
                Err(cancelled())
            } else {
                Ok(())
            }
        });
        self.work(move |state| {
            state.replace_allowed(p.discard_changes)?;
            check()?;
            let expected = float_job::digest(&p.expect_assessment_sha256)?;
            float_job::digest(&p.expect_input_sha256)?;
            float_job::check_losses(&p.allow_losses)?;
            let empty = codec_worker::owner();
            let captured = native_codec::owner(state, empty);
            let prepared = float_job::prepare(
                &PathBuf::from(p.path),
                p.settings,
                Some(p.expect_input_sha256),
                captured,
                native_codec::preparation_budget(),
                check.clone(),
                Frontend::Studio,
            )?;
            let inspection = core_json(&prepared.inspection()?)?;
            let adopted = prepared.adopt(expected, &p.allow_losses)?;
            adopted.check()?;
            if adopted.owner != native_codec::owner(state, empty) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Float import owner changed",
                ));
            }
            let raw_sha = adopted.value.sha256;
            let mut session = Session::new(adopted.value.document);
            session.mark_unsaved();
            let mut candidate = State {
                metadata_realm: state.metadata_realm.clone(),
                session: Some(session),
                import_source: Some(adopted.value.input),
                import_identity: Some(adopted.value.identity),
                ..Default::default()
            };
            let mut value = candidate.inspection()?;
            value["conversion"] = inspection;
            // Complete all fallible receipt/source work before replacing any old binding.
            candidate
                .import_identity
                .as_ref()
                .ok_or_else(|| invalid("Float source identity missing"))?
                .verify_bytes(
                    candidate
                        .import_source
                        .as_ref()
                        .ok_or_else(|| invalid("Float source path missing"))?,
                    adopted.length,
                    &raw_sha,
                    &*check,
                )?;
            check()?;
            candidate.selected_brush = state.selected_brush.take();
            *state = candidate;
            Ok(reply(value))
        })
        .await
    }
}
