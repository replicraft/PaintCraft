//! Each interface owns its document, history, canvas view and tool state.
use super::*;

#[derive(Debug)]
pub(super) struct Parked {
    editor: Editor,
    canvas: Canvas,
    controls: Controls,
    workspace: Workspace,
    text: crate::text_ui::TextUi,
    name: String,
    zoom: f32,
}

impl WebApp {
    pub(super) fn clear_color(&self) -> paintcraft_core::types::Rgba {
        if self.frontend == Frontend::Studio {
            paintcraft_core::types::Rgba::TRANSPARENT
        } else {
            paintcraft_tools::commands::canvas_color(self.editor.session.document())
        }
    }
    pub(crate) fn studio_canvas_scope(&self) -> egui::Id {
        egui::Id::unique((
            self.editor.session.history_ticket().owner,
            self.zoom.to_bits(),
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
    pub(super) fn switch_frontend(&mut self, ctx: &egui::Context, frontend: Frontend) {
        if frontend == self.frontend {
            return;
        }
        let mut other = if let Some(parked) = self.parked.take() {
            parked
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
                    return;
                }
            };
            Box::new(Parked {
                editor,
                canvas: Canvas::default(),
                controls: Controls {
                    tool: Tool::Brush,
                    options: ToolOptions::default(),
                    primary: paintcraft_core::types::Rgba([0, 0, 0, 255]),
                    secondary: WHITE,
                },
                workspace: Workspace::default(),
                text: crate::text_ui::TextUi {
                    style: paintcraft_core::types::text::TextOptions {
                        family: self.preferences.default_font.clone(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                name: "Untitled".into(),
                zoom: 1.0,
            })
        };
        std::mem::swap(&mut self.editor, &mut other.editor);
        std::mem::swap(&mut self.canvas, &mut other.canvas);
        std::mem::swap(&mut self.controls, &mut other.controls);
        std::mem::swap(&mut self.text, &mut other.text);
        std::mem::swap(&mut self.workspace, &mut other.workspace);
        std::mem::swap(&mut self.name, &mut other.name);
        std::mem::swap(&mut self.zoom, &mut other.zoom);
        self.parked = Some(other);
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
        ctx.request_repaint();
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
