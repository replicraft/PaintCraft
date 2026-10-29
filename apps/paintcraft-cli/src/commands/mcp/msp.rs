//! Exact selected MSP version under the current serialized session/source owner.
use super::{
    super::{msp_export, msp_options},
    state::{State, core_json, invalid},
};
use paintcraft_core::types::Result;
use paintcraft_formats::registry::Frontend;
use rmcp::model::{CallToolResult, ContentBlock};
use std::path::Path;
pub(super) fn export(
    state: &State,
    path: &Path,
    overwrite: bool,
    version: msp_options::Version,
    allowed: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<CallToolResult> {
    export_with_metadata_policy(
        state,
        path,
        overwrite,
        version,
        allowed,
        check,
        paintcraft_formats::export::metadata_policy::Intent::Refuse,
    )
}
pub(super) fn export_with_metadata_policy(
    state: &State,
    path: &Path,
    overwrite: bool,
    version: msp_options::Version,
    allowed: &[String],
    check: &dyn Fn() -> Result<()>,
    intent: paintcraft_formats::export::metadata_policy::Intent,
) -> Result<CallToolResult> {
    check()?;
    msp_export::admit(Frontend::Studio, path, version)?;
    let doc = state
        .session
        .as_ref()
        .ok_or_else(|| invalid("create or open a document first"))?
        .document();
    let protect = |destination: &Path| state.protect_copy_sources(destination);
    let policy = paintcraft_formats::export::metadata_policy::Policy::capture(
        doc,
        paintcraft_formats::export::metadata_policy::Target::new(
            paintcraft_formats::registry::Family::Msp,
            paintcraft_formats::registry::Operation::ExportFlat,
        ),
        intent,
    )?;
    let (receipt, ok, _, details) = msp_export::write_copy_with_metadata_policy(
        doc,
        version,
        path,
        overwrite,
        allowed,
        None,
        super::native_codec::preparation_budget(),
        &protect,
        check,
        policy,
    )?;
    let value =
        paintcraft_core::types::json::Value::object([("export", details), ("save", receipt)]);
    let content = vec![ContentBlock::text(core_json(&value)?.to_string())];
    Ok(if ok {
        CallToolResult::success(content)
    } else {
        CallToolResult::error(content)
    })
}
