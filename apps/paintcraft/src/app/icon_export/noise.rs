//! Noise-specific admitted background path; ordinary icon ABI remains unchanged.
use super::*;
impl State {
    pub(super) fn prepare_noise(
        &mut self,
        ctx: &egui::Context,
        budget: &paintcraft_runtime::SharedBudget,
    ) {
        let same = self
            .background
            .matches(&self.draft.config, self.copy.omit, self.copy.bake);
        if !self.copy.ready() || !same {
            self.cache = Default::default();
            self.bytes = None;
            self.copy.retire();
            self.draft.accepted = false;
            self.choose_when_ready = false;
            self.background.retire();
        }
        if let Some(result) = self.background.poll() {
            match result {
                Ok(asset_output::Update::Prepared(prepared)) => {
                    self.cache.update(
                        ctx,
                        (self.draft.config.clone(), self.copy.omit, self.copy.bake),
                        || Ok(prepared),
                    );
                }
                Ok(asset_output::Update::Encoded(bytes)) => self.bytes = Some(bytes),
                Err(error) => {
                    self.choose_when_ready = false;
                    self.cache.update(
                        ctx,
                        (self.draft.config.clone(), self.copy.omit, self.copy.bake),
                        || Err(error.to_string()),
                    );
                }
            }
        }
        if !self.copy.ready()
            || self.background.busy()
            || self
                .background
                .matches(&self.draft.config, self.copy.omit, self.copy.bake)
        {
            return;
        }
        let result = (|| {
            let bytes = asset_output::quote::<icons::Prepared>(
                &self.copy.frozen.document,
                self.draft
                    .config
                    .entries
                    .iter()
                    .map(|e| (u32::from(e.width), u32::from(e.height))),
                icons::ENCODED_BYTES_MAX as u64,
            )?;
            // Admission precedes cloned config/specs/control and worker allocation.
            let lease = budget.reserve(bytes)?;
            let policies = self
                .copy
                .prepare(self.draft.config.clone(), self.target())?;
            let config = self.draft.config.clone();
            self.background.start(
                asset_output::Request {
                    config: &self.draft.config,
                    omit: self.copy.omit,
                    bake: self.copy.bake,
                    source: self.copy.frozen.document.clone(),
                    lease,
                },
                ctx,
                move |doc, check| {
                    icons::prepare_document_with_policies(
                        doc,
                        kind(&config),
                        &specs(&config),
                        if config.filter == widget::Filter::Nearest {
                            icons::Filter::Nearest
                        } else {
                            icons::Filter::Lanczos3
                        },
                        icons::WriteLimits::default(),
                        check,
                        policies,
                    )
                },
                |prepared, losses, check| prepared.encode(losses, check),
            )
        })();
        if let Err(error) = result {
            self.cache.update(
                ctx,
                (self.draft.config.clone(), self.copy.omit, self.copy.bake),
                || Err(error.to_string()),
            );
        }
    }
}
impl App {
    pub(super) fn start_icon_export_write(&mut self, ctx: &egui::Context, mut state: State) {
        let result = (|| {
            let (path, expected) = state.destination.as_ref().ok_or_else(asset_output::limit)?;
            self.guard_export_destination(path)?;
            state.copy.validate(&state.draft.config, state.target())?;
            let owner = state.background.retention()?;
            let bytes = state.bytes.take().ok_or_else(asset_output::limit)?;
            asset_output::WriteJob::start(
                bytes,
                owner,
                path.clone(),
                expected.clone(),
                icons::ENCODED_BYTES_MAX as u64,
                ctx,
            )
        })();
        match result {
            Ok(job) => {
                state.writing = Some(job);
                self.icon_export = Some(state);
                self.confirm = Some(Confirm::IconExport);
            }
            Err(error) => self.editor.set_status(format!("Cannot export: {error}")),
        }
    }
    pub(super) fn poll_icon_export_write(&self, state: &mut State) -> Option<String> {
        let job = state.writing.as_ref()?;
        match job.poll()? {
            asset_output::WriteEvent::Authorize(resolved) => {
                let valid = (|| {
                    if !state.current(self) {
                        return Err(asset_output::limit());
                    }
                    state.copy.validate(&state.draft.config, state.target())?;
                    let (path, _) = state.destination.as_ref().ok_or_else(asset_output::limit)?;
                    self.guard_export_destination(path)?;
                    if let Some(path) = resolved {
                        self.guard_export_destination(&path)?;
                    }
                    Ok(())
                })();
                job.authorize(valid.is_ok());
                None
            }
            asset_output::WriteEvent::Failed(error) => Some(format!("Cannot export: {error}")),
            asset_output::WriteEvent::Done(outcome) => Some(if outcome.is_success() {
                format!("Exported {}", state.destination.as_ref()?.0.display())
            } else {
                format!("Not exported safely: {}", outcome.reasons.join("; "))
            }),
        }
    }
}
