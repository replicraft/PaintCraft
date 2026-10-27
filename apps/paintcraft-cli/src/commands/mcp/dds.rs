//! Explicit DDS textures reuse the current serialized session and shared budget.
use super::{
    super::{dds_export, dds_options},
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
    options: dds_options::Options,
    allowed: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<CallToolResult> {
    check()?;
    dds_export::admit(Frontend::Studio, path)?;
    let doc = state
        .session
        .as_ref()
        .ok_or_else(|| invalid("create or open a document first"))?
        .document();
    // The serialized State lock remains held; owner/session cannot change while
    // the borrowed document and captured source identities guard this copy.
    let protect = |destination: &Path| state.protect_copy_sources(destination);
    let (receipt, ok, _, details) = dds_export::write_copy(
        doc,
        options.resolve(doc.canvas.0, doc.canvas.1)?,
        path,
        overwrite,
        allowed,
        None,
        super::native_codec::preparation_budget(),
        &protect,
        check,
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
