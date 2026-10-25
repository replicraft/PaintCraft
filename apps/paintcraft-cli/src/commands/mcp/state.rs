use super::super::*;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value as Json, json};

#[derive(Default)]
pub struct State {
    pub session: Option<Session>,
    pub source: Option<(PathBuf, String)>,
    pub import_source: Option<PathBuf>,
}

pub fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
pub fn reply(value: Json) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(value.to_string())])
}
pub fn core_json(value: &Value) -> Result<Json> {
    serde_json::from_slice(&paintcraft_core::types::json::canonical(value))
        .map_err(|_| invalid("cannot serialize engine result"))
}
impl State {
    pub fn session(&mut self) -> Result<&mut Session> {
        self.session
            .as_mut()
            .ok_or_else(|| invalid("create or open a document first"))
    }
    pub fn replace_allowed(&self, discard: bool) -> Result<()> {
        if !discard && self.session.as_ref().is_some_and(Session::is_dirty) {
            return Err(invalid("save first or explicitly set discard_changes"));
        }
        Ok(())
    }
    pub fn inspect(&mut self) -> Result<CallToolResult> {
        let session = self.session()?;
        let doc = session.document();
        Ok(reply(json!({
            "document_id":doc.document_id.to_string(), "state_id":doc.state_id.to_string(),
            "width":doc.canvas.0,"height":doc.canvas.1,"dirty":session.is_dirty(),
            "undo_depth":session.undo_depth(),"redo_depth":session.redo_depth(),
            "layers":doc.layers_in_order().map(|l|json!({"id":l.id.to_string(),"name":l.name,"visible":l.visible,"opacity":l.opacity})).collect::<Vec<_>>()
        })))
    }
    pub fn layer(&mut self, id: Option<String>) -> Result<Json> {
        let id = id
            .or_else(|| {
                self.session
                    .as_ref()?
                    .document()
                    .roots
                    .last()
                    .map(ToString::to_string)
            })
            .ok_or_else(|| invalid("document has no layers"))?;
        Ok(json!({"id":id}))
    }
    pub fn apply(&mut self, operations: Vec<Json>) -> Result<CallToolResult> {
        let session = self.session()?;
        let ticket = session.ticket();
        let batch = json!({"schema":"paintcraft.batch/v2","atomic":true,"policy":POLICY,
            "base":{"state_id":ticket.state_id.to_string(),"publication_epoch":ticket.publication_epoch.to_string()},"operations":operations});
        let bytes = serde_json::to_vec(&batch).map_err(|_| invalid("invalid operations"))?;
        if bytes.len() as u64 > OPERATIONS_MAX {
            return Err(invalid("batch exceeds size limit"));
        }
        let receipt = session.apply_json(&bytes)?;
        Ok(reply(core_json(&receipt.to_value())?))
    }
    pub fn save(&mut self, path: PathBuf, overwrite: bool) -> Result<CallToolResult> {
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pcnd"))
        {
            return Err(invalid("project saves require a .pcnd filename"));
        }
        if self
            .import_source
            .as_ref()
            .is_some_and(|source| same_file(source, &path))
        {
            return Err(invalid(
                "a converted project must not replace its original source",
            ));
        }
        let source = self.source.clone();
        let inplace = source
            .as_ref()
            .is_some_and(|(old, _)| same_file(old, &path));
        let session = self.session()?;
        let ctx = Context {
            operation: if inplace { "save" } else { "save-as" },
            source_state: session.document().state_id,
            source_sha256: source.filter(|_| inplace).map(|(_, sha)| sha),
            losses_acknowledged: vec![],
            destination: &path,
        };
        let (receipt, ok, _) = write_native(&path, overwrite || inplace, session.document(), &ctx);
        let result = core_json(&receipt)?;
        if ok {
            let sha = result
                .get("staged_sha256")
                .and_then(Json::as_str)
                .ok_or_else(|| invalid("saved receipt is missing its digest"))?
                .to_owned();
            session.mark_saved();
            self.source = Some((path, sha));
        }
        let content = vec![ContentBlock::text(result.to_string())];
        Ok(if ok {
            CallToolResult::success(content)
        } else {
            CallToolResult::error(content)
        })
    }
}
