//! Each interface owns its document, history, canvas view and tool state.
use super::*;
use paintcraft_ui::t;
mod owners;
pub(super) type Workspaces = owners::Owners<Frontend, Box<Parked>>;
const INITIAL_STATUS: &str = "Ready - Save downloads a project copy; work stays in this tab";

#[derive(Debug)]
pub(super) struct Parked {
    editor: Editor,
    canvas: Canvas,
    controls: Controls,
    workspace: Workspace,
    text: crate::text_ui::TextUi,
    name: String,
    zoom: f32,
    message: String,
}

impl WebApp {
    pub(super) fn clear_color(&self) -> paintcraft_core::types::Rgba {
        if self.frontend == Frontend::Studio {
            paintcraft_core::types::Rgba::TRANSPARENT
        } else {
            paintcraft_tools::commands::canvas_color(self.editor.session.document())
        }
    }
    /// Retained navigation geometry belongs to one document session. Zoom is
    /// only part of the request scope, so anchors survive within this identity.
    pub(crate) fn studio_canvas_id(&self) -> egui::Id {
        egui::Id::unique((
            "web-studio-canvas",
            self.editor.session.history_ticket().owner,
        ))
    }
    pub(crate) fn studio_canvas_scope(&self) -> egui::Id {
        egui::Id::unique((
            self.editor.session.history_ticket().owner,
            self.zoom.to_bits(),
            self.frontend as u8,
            self.variant as u8,
        ))
    }
    pub(super) fn studio_windows(&mut self, ctx: &egui::Context) {
        let enabled = !self.blocked();
        let actions = paintcraft_studio::floating_controls(
            ctx,
            &mut self.controls,
            &mut self.studio,
            enabled,
            egui::Id::unique("web-studio-controls"),
        );
        for action in actions {
            self.action(ctx, action);
        }
        self.workspace.layers = self.studio.layers;
        let view = self.editor.session.history_view();
        let rows: Vec<_> = view
            .entries
            .iter()
            .map(|entry| paintcraft_studio::HistoryRow {
                label: entry.label.text(),
                current: entry.current,
                redo: entry.redo,
            })
            .collect();
        let enabled =
            enabled && !self.editor.tools.gesture_in_progress() && !self.editor.has_text();
        if let Some(index) = paintcraft_studio::history_window(
            ctx,
            &mut self.studio.history,
            &rows,
            view.trimmed_total,
            enabled,
            self.studio.canvas_top,
        ) {
            if let Err(error) = self
                .editor
                .session
                .navigate_history(view.ticket, view.entries[index].history_id)
            {
                self.message = error.to_string();
            }
            self.canvas.invalidate();
        }
    }
    pub(super) fn discard_and_replace(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm_replace else {
            return;
        };
        // Interface transfer needs the explicit discard identity through commit.
        // Other actions retain their original take-before-open/new semantics.
        if !matches!(action, Action::ChooseFrontend(_)) {
            self.confirm_replace = None;
        }
        self.replace(ctx, action);
    }
    pub(super) fn replace_frontend(&mut self, ctx: &egui::Context, frontend: Frontend) {
        let discard =
            self.confirm_replace == Some(Action::ChooseFrontend(frontend)) && self.unsaved();
        let switched = if discard {
            self.switch_frontend_checked(ctx, frontend, true)
        } else {
            self.switch_frontend(ctx, frontend)
        };
        if switched {
            self.confirm_replace = None;
        }
    }
    pub(super) fn switch_frontend(&mut self, ctx: &egui::Context, frontend: Frontend) -> bool {
        self.switch_frontend_checked(ctx, frontend, false)
    }
    pub(super) fn switch_frontend_checked(
        &mut self,
        ctx: &egui::Context,
        frontend: Frontend,
        discard: bool,
    ) -> bool {
        if frontend == self.frontend {
            return true;
        }
        let old = self.frontend;
        let mut transfer = match self.parked.prepare(old, frontend) {
            Ok(transfer) => transfer,
            Err(_) => {
                self.message = t!(
                    ctx,
                    "Could not switch interfaces. Your current workspace is unchanged."
                );
                return false;
            }
        };
        let outgoing_size = if old == Frontend::Studio {
            (800, 600)
        } else {
            self.preferences.canvas.new_size()
        };
        let clean_outgoing = if discard {
            match Editor::new(outgoing_size.0, outgoing_size.1) {
                Ok(editor) => Some(editor),
                Err(error) => {
                    self.message = error.to_string();
                    return false;
                }
            }
        } else {
            None
        };
        let incoming = if transfer.has_destination() {
            None
        } else {
            let size = if frontend == Frontend::Studio {
                (800, 600)
            } else {
                self.preferences.canvas.new_size()
            };
            let editor = match Editor::new(size.0, size.1) {
                Ok(editor) => editor,
                Err(error) => {
                    self.message = error.to_string();
                    return false;
                }
            };
            Some(Box::new(Parked {
                editor,
                canvas: Canvas::default(),
                controls: Controls {
                    tool: Tool::Brush,
                    options: ToolOptions::default(),
                    primary: paintcraft_core::types::Rgba([0, 0, 0, 255]),
                    secondary: WHITE,
                },
                workspace: Workspace::default(),
                text: crate::text_ui::TextUi::with_style(
                    paintcraft_core::types::text::TextOptions {
                        family: self.preferences.default_font.clone(),
                        ..Default::default()
                    },
                ),
                name: "Untitled".into(),
                zoom: 1.0,
                message: INITIAL_STATUS.into(),
            }))
        };
        // All fallible construction and owner-table reservation completed.
        let mut other = transfer
            .take_destination()
            .or(incoming)
            .expect("prepared destination");
        let revoke_composition = self.text.is_composing() || self.text.composition_was_revoked();
        self.text.reset_composition();
        // Contacts belong to the live input stream, never a parked document.
        // Carry only their retired tails into the incoming interface.
        let mut touch = std::mem::take(&mut self.canvas.touch);
        touch.retire_owner(ctx);
        self.editor.cancel_floating();
        self.editor.tools.cancel_gesture();
        if let Some(editor) = clean_outgoing {
            self.editor = editor;
            self.canvas = Canvas::default();
            self.name = "Untitled".into();
            self.message = INITIAL_STATUS.into();
        }
        std::mem::swap(&mut self.editor, &mut other.editor);
        std::mem::swap(&mut self.canvas, &mut other.canvas);
        self.canvas.touch = touch;
        std::mem::swap(&mut self.controls, &mut other.controls);
        std::mem::swap(&mut self.text, &mut other.text);
        if revoke_composition {
            self.text.refuse_previous_composition();
        }
        std::mem::swap(&mut self.workspace, &mut other.workspace);
        std::mem::swap(&mut self.name, &mut other.name);
        std::mem::swap(&mut self.zoom, &mut other.zoom);
        std::mem::swap(&mut self.message, &mut other.message);
        transfer.finish(other);
        self.frontend = frontend;
        self.paste = Default::default();
        self.editor.tools.cancel_gesture();
        paintcraft_ui::theme::apply(ctx, ctx.theme());
        if frontend == Frontend::Studio {
            paintcraft_studio::apply_style_for_theme(ctx, ctx.theme());
        } else {
            paintcraft_classic::apply_style_for_theme(ctx, self.variant, ctx.theme());
        }
        self.update_title();
        if discard
            && old == Frontend::Classic
            && self.preferences.canvas.last != Some(outgoing_size)
        {
            self.preferences.canvas.last = Some(outgoing_size);
            self.persist_preferences();
        }
        ctx.request_repaint();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_interface_menu_opens_studio_chrome_and_restores_classic() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        h.get_by_label("Interface").click();
        h.run();
        h.get_by_label("Studio").click();
        h.run();
        assert_eq!(h.state().frontend, Frontend::Studio);
        assert!(h.query_by_label("Window").is_some());
        assert!(h.query_by_label("History").is_some());
        assert!(h.state().canvas.controls.is_none());
        h.get_by_label("Interface").click();
        h.run();
        h.get_by_label("Classic").click();
        h.run();
        assert_eq!(h.state().frontend, Frontend::Classic);
        assert!(h.query_by_label("Window").is_none());
    }

    #[test]
    fn frontend_switches_use_independent_documents_and_views_and_guard_unsaved_work() {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        let classic = app.editor.session.history_ticket().owner;
        app.zoom = 2.0;
        app.action(&ctx, Action::ChooseFrontend(Frontend::Studio));
        assert_eq!(app.frontend, Frontend::Studio);
        let studio = app.editor.session.history_ticket().owner;
        assert_ne!(studio, classic);
        assert_eq!(app.editor.session.document().canvas, (800, 600));
        assert_eq!(app.zoom, 1.0);
        app.action(&ctx, Action::ChooseFrontend(Frontend::Classic));
        assert_eq!(app.editor.session.history_ticket().owner, classic);
        assert_eq!(app.zoom, 2.0);
        app.edit(Edit::ClearCanvas {
            color: paintcraft_core::types::Rgba([0; 4]),
        });
        app.action(&ctx, Action::ChooseFrontend(Frontend::Studio));
        assert_eq!(app.frontend, Frontend::Classic);
        assert_eq!(
            app.confirm_replace,
            Some(Action::ChooseFrontend(Frontend::Studio))
        );
        app.replace(&ctx, Action::ChooseFrontend(Frontend::Studio));
        assert_eq!(app.editor.session.history_ticket().owner, studio);
        app.action(&ctx, Action::ChooseFrontend(Frontend::Classic));
        assert_eq!(
            app.editor.session.undo_depth(),
            0,
            "discarded edits must not return"
        );
        assert_ne!(app.editor.session.history_ticket().owner, classic);
    }
}

#[cfg(test)]
mod touch_tests {
    use super::*;
    use crate::touch_navigation_tests::{canvas_point, end, start};
    use egui::{Event, Modifiers, PointerButton};
    use egui_kittest::Harness;

    #[test]
    fn touch_interface_swap_retires_strokes_tails_and_focus_ghosts() {
        let mut h = Harness::builder()
            .with_size(egui::vec2(600.0, 800.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        let p = canvas_point(&h, 0.0);
        let classic = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        start(&mut h, 1, p);
        assert!(h.state().editor.tools.gesture_in_progress());
        let ctx = h.ctx.clone();
        h.state_mut().switch_frontend(&ctx, Frontend::Studio);
        h.step();
        assert!(
            !h.state()
                .parked
                .get(Frontend::Classic)
                .unwrap()
                .editor
                .tools
                .gesture_in_progress()
        );
        let studio = h.state().editor.session.ticket();
        end(&mut h, 1, p);
        h.run();
        assert_eq!(h.state().editor.session.ticket(), studio);
        h.state_mut().switch_frontend(&ctx, Frontend::Classic);
        h.run();
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert!(std::sync::Arc::ptr_eq(
            &classic,
            &h.state().editor.session.snapshot()
        ));
        let p = canvas_point(&h, 0.0);
        start(&mut h, 2, p);
        h.input_mut().focused = false;
        h.input_mut().events.push(Event::WindowFocused(false));
        h.step();
        h.input_mut().focused = true;
        h.input_mut().events.push(Event::WindowFocused(true));
        h.step();
        // The backend touch inventory still contains id2; our retired raw state
        // must not turn it into an interface-wide quarantine on either swap.
        h.state_mut().switch_frontend(&ctx, Frontend::Studio);
        h.step();
        h.state_mut().switch_frontend(&ctx, Frontend::Classic);
        h.run();
        let p = canvas_point(&h, 0.0);
        h.input_mut().events.extend([
            Event::PointerMoved(p),
            Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.step();
        h.input_mut().events.push(Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        h.run();
        assert_ne!(h.state().editor.session.ticket(), ticket);
    }
}

#[cfg(test)]
#[path = "interfaces/ime_tests.rs"]
mod ime_tests;

#[cfg(test)]
#[path = "interfaces/keyed_tests.rs"]
mod keyed_tests;
