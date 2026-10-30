//! Canvas composition is presentation only until an actual result is accepted.
//! Framework events have no native composition ID. Known revoked results are
//! refused; a new Preedit is the framework's observable new-start boundary.
use super::*;
use paintcraft_ui::t;
use std::ops::Range;

#[derive(Debug, Default)]
pub(crate) struct State {
    binding: Option<Binding>,
    refuse_unbound: bool,
    pub(super) selected_all: bool,
    pub(super) interrupt: bool,
}
#[derive(Debug)]
struct Binding {
    session: Uuid,
    document: (Uuid, u64),
    experience: Experience,
    target: (u64, Uuid),
    revision: paintcraft_tools::PreviewRevision,
    base: String,
    preedit: String,
    selected_all: bool,
}
impl App {
    fn canvas_ime_matches(&self, binding: &Binding) -> bool {
        binding.session == self.editor.session_owner_id()
            && binding.document == self.editor.render_key()
            && binding.experience == self.experience()
            && Some(binding.target) == self.editor.canvas_text_target()
            && binding.revision == self.editor.canvas_text_revision()
            && self.editor.canvas_text_content() == Some(binding.base.as_str())
            && self.editor.target() == Some(binding.target.1)
    }
    pub(super) fn revoke_canvas_ime(&mut self) {
        if self.canvas_ime.binding.take().is_some() {
            self.canvas_ime.refuse_unbound = true;
            self.canvas_ime.interrupt = true;
        }
        self.canvas_ime.selected_all = false;
        self.ime_composing = false;
    }
    pub(super) fn guard_canvas_ime(&mut self, ctx: &egui::Context) {
        if self
            .canvas_ime
            .binding
            .as_ref()
            .is_some_and(|binding| !self.canvas_ime_matches(binding))
            || self.action_context().blocked
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
            || self.picking.is_some()
        {
            self.revoke_canvas_ime();
        }
    }
    fn bind_canvas_ime(&self) -> std::result::Result<Binding, &'static str> {
        let target = self
            .editor
            .canvas_text_target()
            .ok_or("Start a canvas text draft before composing")?;
        let text = self.editor.canvas_text_content().unwrap_or_default();
        let mut base = String::new();
        base.try_reserve_exact(text.len())
            .map_err(|_| "Not enough memory for text composition")?;
        base.push_str(text);
        Ok(Binding {
            session: self.editor.session_owner_id(),
            document: self.editor.render_key(),
            experience: self.experience(),
            target,
            revision: self.editor.canvas_text_revision(),
            base,
            preedit: String::new(),
            selected_all: self.canvas_ime.selected_all,
        })
    }
    /// Runs in the same ordered canvas loop as pointer creation of text drafts.
    /// Return true only when canvas text owns this event.
    pub(super) fn canvas_text_event(&mut self, ctx: &egui::Context, event: &egui::Event) -> bool {
        self.guard_canvas_ime(ctx);
        if matches!(event, egui::Event::WindowFocused(false)) {
            self.revoke_canvas_ime();
            if !matches!(self.editor.tool, Tool::Magnifier | Tool::Pan) {
                self.editor.commit_text();
            }
            return false;
        }
        if matches!(event, egui::Event::PointerButton { pressed: true, .. }) {
            self.revoke_canvas_ime();
            return false;
        }
        if !self.editor.is_editing_text()
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
            || self.picking.is_some()
            || self.action_context().blocked
        {
            return false;
        }
        let result = match event {
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars,
            }) => self.canvas_preedit(text, active_range_chars.clone()),
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => self.canvas_ime_commit(text),
            egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                before_chars,
                after_chars,
            }) => {
                if self.canvas_ime.binding.is_none() && self.canvas_ime.refuse_unbound {
                    Err("IME deletion ignored because its text draft changed")
                } else if *after_chars != 0 {
                    Err("Canvas text supports deletion before its end cursor only")
                } else {
                    self.canvas_replace(*before_chars, "")
                }
            }
            egui::Event::Text(text) | egui::Event::Paste(text)
                if self.canvas_ime.binding.is_none() =>
            {
                self.canvas_replace(
                    if self.canvas_ime.selected_all {
                        self.editor
                            .canvas_text_content()
                            .unwrap_or_default()
                            .chars()
                            .count()
                    } else {
                        0
                    },
                    text,
                )
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                if self.canvas_ime.binding.is_some() {
                    if *key == egui::Key::Escape {
                        self.revoke_canvas_ime();
                    }
                    return matches!(
                        key,
                        egui::Key::Escape
                            | egui::Key::Enter
                            | egui::Key::Backspace
                            | egui::Key::Delete
                    );
                }
                match key {
                    egui::Key::A if modifiers.command => {
                        self.canvas_ime.selected_all = true;
                        Ok(())
                    }
                    egui::Key::Backspace => {
                        let count = self
                            .editor
                            .canvas_text_content()
                            .unwrap_or_default()
                            .chars()
                            .count();
                        self.canvas_replace(
                            if self.canvas_ime.selected_all {
                                count
                            } else {
                                usize::from(count > 0)
                            },
                            "",
                        )
                    }
                    egui::Key::Delete if self.canvas_ime.selected_all => self.canvas_replace(
                        self.editor
                            .canvas_text_content()
                            .unwrap_or_default()
                            .chars()
                            .count(),
                        "",
                    ),
                    egui::Key::Enter if modifiers.command => {
                        self.editor.place_text();
                        Ok(())
                    }
                    egui::Key::Enter => self.canvas_replace(0, "\n"),
                    _ => return false,
                }
            }
            _ => return false,
        };
        if let Err(message) = result {
            self.editor.set_status(t!(ctx, message));
        }
        ctx.request_repaint();
        true
    }
    fn reject_canvas_preedit(
        &mut self,
        message: &'static str,
    ) -> std::result::Result<(), &'static str> {
        self.revoke_canvas_ime();
        self.canvas_ime.refuse_unbound = true;
        self.canvas_ime.interrupt = true;
        Err(message)
    }
    fn canvas_preedit(
        &mut self,
        text: &str,
        range: Option<Range<usize>>,
    ) -> std::result::Result<(), &'static str> {
        if text.is_empty() && self.canvas_ime.binding.is_none() {
            // An old backend cleanup notification cannot authenticate a new
            // target or undo a known revoked-result quarantine.
            return Ok(());
        }
        let count = text.chars().count();
        if count > paintcraft_raster::font::TEXT_CHARS_MAX
            || text.chars().any(|c| c != '\n' && c.is_control())
        {
            return self.reject_canvas_preedit(
                "Text composition exceeds the draft limit or contains unsupported characters",
            );
        }
        if range
            .as_ref()
            .is_some_and(|r| r.start > r.end || r.end != count)
        {
            return self.reject_canvas_preedit(
                "Canvas IME currently supports an end cursor; this composition range is unsupported",
            );
        }
        let mut binding = match self.canvas_ime.binding.take() {
            Some(binding) => binding,
            None => self.bind_canvas_ime()?,
        };
        let keep = if binding.selected_all {
            0
        } else {
            binding.base.chars().count()
        };
        if keep + count > paintcraft_raster::font::TEXT_CHARS_MAX {
            return self.reject_canvas_preedit(
                "Text composition exceeds the draft limit; no text was truncated",
            );
        }
        let mut preedit = String::new();
        if preedit.try_reserve_exact(text.len()).is_err() {
            return self.reject_canvas_preedit("Not enough memory for text composition");
        }
        preedit.push_str(text);
        binding.preedit = preedit;
        self.canvas_ime.binding = Some(binding);
        self.canvas_ime.refuse_unbound = false;
        self.canvas_ime.interrupt = false;
        self.ime_composing = true; // Empty preedit may precede the real Commit.
        Ok(())
    }
    fn canvas_ime_commit(&mut self, text: &str) -> std::result::Result<(), &'static str> {
        if self.canvas_ime.binding.is_none() && self.canvas_ime.refuse_unbound {
            return Err(
                "IME result ignored because the canvas text draft changed; restart composition",
            );
        }
        // Some framework backends send only Commit. Accept a first direct result
        // for the current exact draft, but never after a known revoked lifetime.
        let was_bound = self.canvas_ime.binding.is_some();
        let binding = match self.canvas_ime.binding.take() {
            Some(b) => b,
            None => self.bind_canvas_ime()?,
        };
        if !self.canvas_ime_matches(&binding) {
            self.canvas_ime.refuse_unbound = true;
            return Err("IME result belongs to an older text draft");
        }
        if text.is_empty() {
            // Empty completion removes provisional presentation; it is not
            // authority to erase a previously selected committed block.
            self.canvas_ime.selected_all = false;
            self.ime_composing = false;
            return Ok(());
        }
        let delete = if binding.selected_all {
            binding.base.chars().count()
        } else {
            0
        };
        if self
            .editor
            .replace_canvas_text_tail(binding.target.0, delete, text)
            .is_err()
        {
            if was_bound {
                self.canvas_ime.binding = Some(binding);
            }
            return Err("IME result could not fit the text draft; no text was changed");
        }
        self.canvas_ime.selected_all = false;
        self.ime_composing = false;
        Ok(())
    }
    fn canvas_replace(
        &mut self,
        delete: usize,
        text: &str,
    ) -> std::result::Result<(), &'static str> {
        if self.canvas_ime.binding.is_some() {
            return Err("Finish the current composition before deleting committed text");
        }
        let (token, _) = self
            .editor
            .canvas_text_target()
            .ok_or("Text draft changed")?;
        self.editor
            .replace_canvas_text_tail(token, delete, text)
            .map_err(|_| "Text could not fit the draft; no text was changed")?;
        self.canvas_ime.selected_all = false;
        Ok(())
    }
    pub(super) fn canvas_text_preview(&self) -> Option<Arc<Preview>> {
        let original = self.editor.preview_snapshot()?;
        let Some(binding) = self
            .canvas_ime
            .binding
            .as_ref()
            .filter(|b| self.canvas_ime_matches(b))
        else {
            return Some(original);
        };
        let mut preview = (*original).clone();
        if let Preview::Text { text, .. } = &mut preview {
            if binding.selected_all {
                text.clear();
            }
            if text.try_reserve_exact(binding.preedit.len()).is_err() {
                return Some(original);
            }
            text.push_str(&binding.preedit);
        }
        Some(Arc::new(preview))
    }
}

#[cfg(test)]
#[path = "canvas_ime/tests.rs"]
mod tests;
