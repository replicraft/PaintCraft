//! One provisional IME lifetime, bound to the exact owner and editable text draft.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use paintcraft_tools::{PreviewRevision, TextStamp};
use paintcraft_ui::actions::Frontend;
use std::{ops::Range, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Key {
    owner: Uuid,
    source: Ticket,
    stamp: TextStamp,
    tool: Tool,
    frontend: Frontend,
    variant: paintcraft_classic::ClassicVariant,
    revision: PreviewRevision,
}
#[derive(Debug)]
struct Binding {
    key: Key,
    source: Arc<Document>,
    preedit: String,
    selected_all: bool,
    deleted: usize,
}
#[derive(Debug, Default)]
pub(super) struct State {
    binding: Option<Binding>,
    revision: PreviewRevision,
    refuse_unbound: bool,
    interrupt: bool,
}
impl TextUi {
    pub(crate) fn preedit(&self) -> &str {
        self.composition
            .binding
            .as_ref()
            .map_or("", |b| b.preedit.as_str())
    }
    pub(crate) fn is_composing(&self) -> bool {
        self.composition.binding.is_some()
    }
    /// Reset presentation only; the original draft remains owned by ToolState.
    pub(crate) fn reset_composition(&mut self) {
        if self.composition.binding.take().is_some() {
            self.composition.revision.advance();
            self.composition.refuse_unbound = true;
            self.composition.interrupt = true;
        }
        self.selected_all = false;
    }
    pub(crate) fn composition_was_revoked(&self) -> bool {
        self.composition.refuse_unbound
    }
    /// Input events belong to the live frontend, not a parked TextUi. Carry
    /// quarantine only, never an old candidate's text, into a different owner.
    pub(crate) fn refuse_previous_composition(&mut self) {
        self.reset_composition();
        self.composition.refuse_unbound = true;
        self.composition.interrupt = true;
        self.composition.revision.advance();
    }
    pub(super) fn interrupt_composition(&self) -> bool {
        self.composition.interrupt
    }
}
// Egui no longer emits Disabled, but explicit older backend events must still
// revoke our lifetime rather than leave a provisional image accepted.
#[allow(deprecated)]
pub(super) fn is_disabled(event: &egui::Event) -> bool {
    matches!(event, egui::Event::Ime(egui::ImeEvent::Disabled))
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn validate_text(text: &str) -> Result<usize> {
    let count = text.chars().count();
    if count > paintcraft_raster::font::TEXT_CHARS_MAX
        || text.chars().any(|c| c != '\n' && c.is_control())
    {
        return Err(invalid(
            "Text composition exceeds the draft limit or contains unsupported characters; no text was truncated",
        ));
    }
    Ok(count)
}
impl WebApp {
    fn composition_matches(&self, binding: &Binding) -> bool {
        binding.key.owner == self.editor.session.owner_id()
            && binding.key.source == self.editor.session.ticket()
            && Arc::ptr_eq(&binding.source, &self.editor.session.snapshot())
            && self.editor.tools.text_stamp() == Some(binding.key.stamp)
            && self.editor.target == Some(binding.key.stamp.layer())
            && self.editor.tools.tool == binding.key.tool
            && self.controls.tool == binding.key.tool
            && self.frontend == binding.key.frontend
            && self.variant == binding.key.variant
    }
    pub(crate) fn guard_composition(&mut self, ctx: &egui::Context) {
        if self
            .text
            .composition
            .binding
            .as_ref()
            .is_some_and(|b| !self.composition_matches(b))
            || self.blocked()
            || self.text.widget_owned_input
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
        {
            self.text.reset_composition();
        }
    }
    pub(crate) fn composition_key(&self) -> Option<Key> {
        self.text
            .composition
            .binding
            .as_ref()
            .filter(|b| self.composition_matches(b))
            .map(|b| b.key)
    }
    fn new_composition(&self) -> Result<Binding> {
        let stamp = self
            .editor
            .tools
            .text_stamp()
            .ok_or_else(|| invalid("Start a canvas text draft before composing"))?;
        if self.editor.target != Some(stamp.layer()) || self.controls.tool != self.editor.tools.tool
        {
            return Err(invalid("Canvas text target changed"));
        }
        Ok(Binding {
            key: Key {
                owner: self.editor.session.owner_id(),
                source: self.editor.session.ticket(),
                stamp,
                tool: self.editor.tools.tool,
                frontend: self.frontend,
                variant: self.variant,
                revision: self.text.composition.revision,
            },
            source: self.editor.session.snapshot(),
            preedit: String::new(),
            selected_all: self.text.selected_all,
            deleted: 0,
        })
    }
    fn set_preedit(&mut self, text: &str, range: Option<Range<usize>>) -> Result<()> {
        if text.is_empty() && !self.text.is_composing() {
            return Ok(());
        }
        let count = validate_text(text)?;
        if range.is_some_and(|r| r.start > r.end || r.end != count) {
            return Err(invalid(
                "Canvas IME supports an end cursor; this composition range is unsupported",
            ));
        }
        let mut binding = if let Some(binding) = self.text.composition.binding.take() {
            binding
        } else {
            self.new_composition()?
        };
        if !self.composition_matches(&binding) {
            return Err(invalid("Text composition belongs to an older draft"));
        }
        let keep = if binding.selected_all {
            0
        } else {
            self.editor
                .tools
                .text_content()
                .unwrap_or_default()
                .chars()
                .count()
                .checked_sub(binding.deleted)
                .ok_or_else(|| invalid("Composition prefix changed"))?
        };
        if keep
            .checked_add(count)
            .is_none_or(|n| n > paintcraft_raster::font::TEXT_CHARS_MAX)
        {
            return Err(invalid(
                "Text composition exceeds the draft limit; no text was truncated",
            ));
        }
        let mut preedit = String::new();
        preedit.try_reserve_exact(text.len()).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Not enough memory for text composition",
            )
        })?;
        preedit.push_str(text);
        self.text.composition.revision.advance();
        binding.key.revision = self.text.composition.revision;
        binding.preedit = preedit;
        self.text.composition.binding = Some(binding);
        self.text.composition.refuse_unbound = false;
        self.text.composition.interrupt = false;
        Ok(())
    }
    fn commit_composition(&mut self, text: &str) -> Result<()> {
        if self.text.composition.refuse_unbound && !self.text.is_composing() {
            return Err(invalid(
                "IME result ignored because its text draft changed; restart composition",
            ));
        }
        validate_text(text)?;
        let binding = self.text.composition.binding.as_ref().map_or_else(
            || self.new_composition(),
            |b| {
                if self.composition_matches(b) {
                    self.new_composition().map(|mut copy| {
                        copy.selected_all = b.selected_all;
                        copy.deleted = b.deleted;
                        copy
                    })
                } else {
                    Err(invalid("IME result belongs to an older text draft"))
                }
            },
        )?;
        if !text.is_empty() {
            let delete = if binding.selected_all {
                self.editor
                    .tools
                    .text_content()
                    .unwrap_or_default()
                    .chars()
                    .count()
            } else {
                binding.deleted
            };
            self.editor
                .tools
                .replace_text_tail(binding.key.stamp.draft_token(), delete, text)?;
        }
        self.text.composition.binding = None;
        self.text.composition.revision.advance();
        self.text.composition.interrupt = false;
        self.text.selected_all = false;
        Ok(())
    }
    /// Both texture and caret use exactly the same real text intention.
    pub(crate) fn canvas_text_preview(&self) -> Result<Option<Preview>> {
        let mut preview = self.editor.tools.preview();
        if let Some(binding) = &self.text.composition.binding {
            if !self.composition_matches(binding) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Composition preview changed",
                ));
            }
            let Some(Preview::Text { text, .. }) = &mut preview else {
                return Err(invalid("Composition has no canvas text draft"));
            };
            if binding.selected_all {
                text.clear();
            } else if binding.deleted != 0 {
                let keep = text
                    .chars()
                    .count()
                    .checked_sub(binding.deleted)
                    .ok_or_else(|| invalid("Composition prefix changed"))?;
                let end = text
                    .char_indices()
                    .nth(keep)
                    .map_or(text.len(), |(at, _)| at);
                text.truncate(end); // Exact requested tail replacement, not input truncation.
            }
            text.try_reserve_exact(binding.preedit.len()).map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Composition preview allocation failed",
                )
            })?;
            text.push_str(&binding.preedit);
        }
        Ok(preview)
    }
    pub(crate) fn canvas_preview_document(&self) -> Result<Option<Document>> {
        if self.text.is_composing() {
            self.editor.display_preview(self.canvas_text_preview()?)
        } else {
            self.editor.display_document()
        }
    }
    /// Eframe's text_agent emits a committed-prefix Delete immediately before
    /// Preedit (or Text for ordinary correction) in one input callback. Consume
    /// that pair before a standalone Delete can mutate the actual draft.
    pub(crate) fn canvas_text_input_pair(
        &mut self,
        ctx: &egui::Context,
        first: &egui::Event,
        next: &egui::Event,
    ) -> bool {
        let egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars,
            after_chars,
        }) = first
        else {
            return false;
        };
        if !matches!(
            next,
            egui::Event::Ime(egui::ImeEvent::Preedit { .. }) | egui::Event::Text(_)
        ) {
            return false;
        }
        self.guard_composition(ctx);
        if self.blocked()
            || self.text.widget_owned_input
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
            || !self.editor.tools.is_editing_text()
        {
            return true;
        }
        let result = (|| {
            if *after_chars != 0 {
                return Err(invalid(
                    "Canvas text supports deletion before its end cursor only",
                ));
            }
            if *before_chars
                > self
                    .editor
                    .tools
                    .text_content()
                    .unwrap_or_default()
                    .chars()
                    .count()
            {
                return Err(invalid("Composition deletion exceeds the original draft"));
            }
            match next {
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text,
                    active_range_chars,
                }) => {
                    let mut binding = self
                        .text
                        .composition
                        .binding
                        .take()
                        .map_or_else(|| self.new_composition(), Ok)?;
                    if !self.composition_matches(&binding) {
                        return Err(invalid("Composition prefix belongs to an older draft"));
                    }
                    let count = self
                        .editor
                        .tools
                        .text_content()
                        .unwrap_or_default()
                        .chars()
                        .count();
                    if *before_chars > count {
                        return Err(invalid("Composition deletion exceeds the original draft"));
                    }
                    binding.deleted = if binding.selected_all {
                        0
                    } else {
                        binding
                            .deleted
                            .checked_add(*before_chars)
                            .filter(|n| *n <= count)
                            .ok_or_else(|| {
                                invalid("Composition deletion exceeds the original draft")
                            })?
                    };
                    self.text.composition.binding = Some(binding);
                    self.set_preedit(text, active_range_chars.clone())
                }
                egui::Event::Text(text) => {
                    if self.text.is_composing() {
                        return Err(invalid(
                            "Finish composition before ordinary text replacement",
                        ));
                    }
                    let delete = if self.text.selected_all {
                        self.editor
                            .tools
                            .text_content()
                            .unwrap_or_default()
                            .chars()
                            .count()
                    } else {
                        *before_chars
                    };
                    self.replace_text_exact(delete, text)
                }
                _ => unreachable!(),
            }
        })();
        if let Err(error) = result {
            self.text.reset_composition();
            self.text.composition.refuse_unbound = true;
            self.text.composition.interrupt = true;
            self.message = error.to_string();
        }
        ctx.request_repaint();
        true
    }
    /// Process IME before normal canvas text keys; a revoked lifetime cannot
    /// authenticate a late result for another draft, even with identical content.
    pub(super) fn composition_event(&mut self, ctx: &egui::Context, event: &egui::Event) -> bool {
        if is_disabled(event) || matches!(event, egui::Event::WindowFocused(false)) {
            self.text.reset_composition();
            return true;
        }
        let result = match event {
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars,
            }) => self.set_preedit(text, active_range_chars.clone()),
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => self.commit_composition(text),
            egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                before_chars,
                after_chars,
            }) => {
                if self.text.is_composing() || self.text.composition.refuse_unbound {
                    Err(invalid(
                        "Finish or restart composition before deleting text",
                    ))
                } else if *after_chars != 0 {
                    Err(invalid(
                        "Canvas text supports deletion before its end cursor only",
                    ))
                } else {
                    self.replace_text_exact(*before_chars, "")
                }
            }
            _ => return false,
        };
        if let Err(error) = result {
            // Invalid input never retains a previous valid provisional payload.
            self.text.reset_composition();
            self.text.composition.refuse_unbound = true;
            self.text.composition.interrupt = true;
            self.message = error.to_string();
        }
        ctx.request_repaint();
        true
    }
    pub(super) fn replace_text_exact(&mut self, delete: usize, text: &str) -> Result<()> {
        let (token, _) = self
            .editor
            .tools
            .text_target()
            .ok_or_else(|| invalid("Text draft changed"))?;
        self.editor.tools.replace_text_tail(token, delete, text)?;
        self.text.selected_all = false;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn test_start_preedit(&mut self, text: &str) {
        if !self.editor.tools.is_editing_text() {
            self.controls.tool = Tool::Text;
            self.editor.tools.choose_tool(Tool::Text);
            self.editor.tools.set_text_style(self.text.style.clone());
            self.editor
                .tools
                .begin_text(self.editor.target.unwrap(), (0, 0));
        }
        self.set_preedit(text, None).unwrap();
    }
}

#[cfg(test)]
mod tests;
