use super::super::*;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value as Json, json};

#[derive(Default)]
pub struct State {
    pub metadata_realm: super::super::source_metadata::Realm,
    pub session: Option<Session>,
    pub source: Option<(PathBuf, String)>,
    pub project_identity: Option<paintcraft_platform::source::SourceIdentity>,
    pub import_source: Option<PathBuf>,
    pub import_identity: Option<paintcraft_platform::source::SourceIdentity>,
    pub import_resources: Vec<PathBuf>,
    pub selected_brush: Option<super::brushes::Selected>,
    pub brush_sources: Vec<std::sync::Arc<super::super::brushes::Loaded>>,
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
    pub fn protect_import_sources(&self, path: &Path) -> Result<()> {
        // One fail-closed identity snapshot across every retained brush source.
        let candidate = paintcraft_platform::source::SourceIdentity::existing_destination(path)?;
        if self.import_identity.as_ref().is_some_and(|source| {
            candidate
                .as_ref()
                .is_some_and(|candidate| source.matches(candidate))
        }) {
            return Err(invalid(
                "output must not replace the captured imported image",
            ));
        }
        if self
            .selected_brush
            .as_ref()
            .is_some_and(|selected| selected.loaded.protects(path, candidate.as_ref()))
            || self
                .brush_sources
                .iter()
                .any(|source| source.protects(path, candidate.as_ref()))
        {
            return Err(invalid("output must not replace an adopted brush source"));
        }

        if self
            .import_source
            .as_ref()
            .is_some_and(|source| same_file(source, path))
            || self
                .import_resources
                .iter()
                .any(|source| same_file(source, path))
        {
            return Err(invalid(
                "a converted project or export must not replace its original source or font resources",
            ));
        }
        Ok(())
    }
    /// Copy routes protect the retained native project too; intentional native
    /// Save uses protect_import_sources and its existing digest checks instead.
    pub fn protect_copy_sources(&self, path: &Path) -> Result<()> {
        self.protect_import_sources(path)?;
        if self.source.is_some() {
            let identity = self.project_identity.as_ref().ok_or_else(|| invalid(
                "captured project identity unavailable; reopen the saved project before exporting"
            ))?;
            if identity.matches_path(path)? {
                return Err(invalid("copy must not replace its captured source project"));
            }
        }
        Ok(())
    }
    /// Recheck captured sources after staging and immediately before publication.
    pub fn write_copy_with(
        &self,
        path: &Path,
        overwrite: bool,
        encode: impl FnOnce(&mut dyn std::io::Write) -> Result<()>,
    ) -> Result<Outcome> {
        self.write_copy_with_checked(path, overwrite, encode, &|| Ok(()))
    }
    /// Recheck request lifetime at the existing post-stage publication barrier.
    /// This retains source and destination checks; it is not an external CAS.
    pub fn write_copy_with_checked(
        &self,
        path: &Path,
        overwrite: bool,
        encode: impl FnOnce(&mut dyn std::io::Write) -> Result<()>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Outcome> {
        check()?;
        self.protect_copy_sources(path)?;
        Ok(save::save_with_preparation(
            path,
            overwrite,
            save::STAGED_FILE_BYTES_MAX,
            encode,
            |_| {
                check()?;
                self.protect_copy_sources(path)
            },
        ))
    }
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
        Ok(reply(self.inspection()?))
    }
    pub fn inspection(&mut self) -> Result<Json> {
        let session = self.session()?;
        let doc = session.document();
        let layers = doc.layers_in_order().map(|layer| {
            let mut value = json!({"id":layer.id.to_string(),"name":layer.name,"visible":layer.visible,"opacity":layer.opacity});
            if !layer.filters.is_empty() {value["filters"] = core_json(&paintcraft_document::filters::inspection(&layer.filters))?;}
            Ok(value)
        }).collect::<Result<Vec<_>>>()?;
        Ok(json!({
            "document_id":doc.document_id.to_string(), "state_id":doc.state_id.to_string(),
            "width":doc.canvas.0,"height":doc.canvas.1,"dirty":session.is_dirty(),
            "undo_depth":session.undo_depth(),"redo_depth":session.redo_depth(),
            "layers":layers
        }))
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
        self.protect_import_sources(&path)?;
        let source = self.source.clone();
        let inplace = source
            .as_ref()
            .is_some_and(|(old, _)| same_file(old, &path));
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| invalid("create or open a document first"))?;
        let ctx = Context {
            operation: if inplace { "save" } else { "save-as" },
            source_state: session.document().state_id,
            source_sha256: source.filter(|_| inplace).map(|(_, sha)| sha),
            losses_acknowledged: vec![],
            destination: &path,
        };
        let (receipt, ok, _) = write_native_guarded(
            &path,
            overwrite || inplace,
            session.document(),
            &ctx,
            || {
                if inplace {
                    self.protect_import_sources(&path)
                } else {
                    self.protect_copy_sources(&path)
                }
            },
        );
        let result = core_json(&receipt)?;
        if ok {
            let sha = result
                .get("staged_sha256")
                .and_then(Json::as_str)
                .ok_or_else(|| invalid("saved receipt is missing its digest"))?
                .to_owned();
            self.session()?.mark_saved();
            // Publication already succeeded. Failure to capture the resulting
            // binding cannot undo that fact: keep the truthful durable receipt,
            // and let copy guards fail closed until the project is reopened.
            self.project_identity =
                paintcraft_platform::source::SourceIdentity::existing_destination(&path)
                    .ok()
                    .flatten();
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

#[cfg(test)]
#[path = "state/cancellation_tests.rs"]
mod cancellation_tests;
#[cfg(test)]
#[path = "state/source_tests.rs"]
mod source_tests;
