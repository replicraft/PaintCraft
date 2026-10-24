//! Frontend-owned canvas dimensions and New-canvas policy use separate staged dialogs.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanvasDraft {
    Studio {
        tab: Uuid,
        revision: Option<(Uuid, u64)>,
        view: paintcraft_studio::CanvasSize,
    },
    Defaults(settings::CanvasDefaults),
    Properties {
        tab: Uuid,
        revision: (Uuid, u64),
        size: (u32, u32),
        name: String,
    },
    Resize {
        tab: Uuid,
        revision: (Uuid, u64),
        size: (u32, u32),
    },
}

impl App {
    pub(super) fn remember_canvas_size(&mut self, size: (u32, u32)) {
        if self.settings.canvas.last != Some(size) {
            self.settings.canvas.last = Some(size);
            self.persist_settings();
        }
    }

    pub(super) fn begin_canvas_dialog(&mut self, ctx: &egui::Context, defaults: bool) {
        self.resolve_tab_input(ctx);
        if !defaults && self.experience() == Experience::Studio {
            self.confirm = Some(Confirm::Canvas(CanvasDraft::Studio {
                tab: self.active_tab_id(),
                revision: (!self.editor.has_pending_edits()).then(|| self.editor.render_key()),
                view: paintcraft_studio::CanvasSize {
                    size: self.editor.document().canvas,
                    anchor: paintcraft_core::types::canvas::Anchor::Center,
                },
            }));
            return;
        }
        let draft = if defaults {
            CanvasDraft::Defaults(self.settings.canvas.clone())
        } else {
            CanvasDraft::Resize {
                tab: self.active_tab_id(),
                revision: self.editor.render_key(),
                size: self.editor.document().canvas,
            }
        };
        self.confirm = Some(Confirm::Canvas(draft));
    }

    pub(super) fn begin_image_properties(&mut self) {
        self.editor.cancel_gesture();
        let name = self
            .editor
            .path()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        self.confirm = Some(Confirm::Canvas(CanvasDraft::Properties {
            tab: self.active_tab_id(),
            revision: self.editor.render_key(),
            size: self.editor.document().canvas,
            name,
        }));
    }

    pub(super) fn canvas_settings_dialog(&mut self, ctx: &egui::Context, mut draft: CanvasDraft) {
        if let CanvasDraft::Studio {
            tab,
            revision,
            view,
        } = draft
        {
            self.studio_canvas_settings_dialog(ctx, tab, revision, view);
            return;
        }
        if let CanvasDraft::Resize { tab, revision, .. }
        | CanvasDraft::Properties { tab, revision, .. } = &draft
            && (*tab != self.active_tab_id() || *revision != self.editor.render_key())
        {
            self.confirm = None;
            self.editor
                .set_status("Image dimensions were not applied: the source document changed.");
            return;
        }
        use paintcraft_classic::canvas_size::{CanvasResponse, CanvasSize, StartupSize};
        let mut view = match &draft {
            CanvasDraft::Studio { .. } => unreachable!("Studio has its own dialog"),
            CanvasDraft::Defaults(prefs) => CanvasSize {
                size: prefs.configured,
                startup: Some(StartupSize {
                    remember_last: prefs.remember_last,
                    last: prefs.last,
                }),
            },
            CanvasDraft::Resize { size, .. } | CanvasDraft::Properties { size, .. } => CanvasSize {
                size: *size,
                startup: None,
            },
        };
        let response = if let CanvasDraft::Properties { name, .. } = &draft {
            paintcraft_classic::canvas_size::image_properties_dialog(
                ctx,
                &mut view.size,
                name,
                settings::SIZE_MAX,
            )
        } else {
            paintcraft_classic::canvas_size::canvas_size_dialog(ctx, &mut view, settings::SIZE_MAX)
        };
        if ctx.current_pass_index() != 0 {
            return;
        }
        match &mut draft {
            CanvasDraft::Studio { .. } => unreachable!("Studio has its own dialog"),
            CanvasDraft::Defaults(prefs) => {
                prefs.configured = view.size;
                prefs.remember_last = view.startup.expect("default-size policy").remember_last;
            }
            CanvasDraft::Resize { size, .. } | CanvasDraft::Properties { size, .. } => {
                *size = view.size
            }
        }
        if response == CanvasResponse::Cancel {
            self.confirm = None;
        } else if response == CanvasResponse::Apply {
            self.confirm = None;
            match draft {
                CanvasDraft::Studio { .. } => unreachable!("Studio has its own dialog"),
                CanvasDraft::Defaults(prefs) => {
                    self.settings.canvas = prefs;
                    self.persist_settings();
                }
                CanvasDraft::Resize { size, .. } | CanvasDraft::Properties { size, .. } => {
                    let before = self.editor.document().canvas;
                    if size == before {
                        return;
                    }
                    self.request(
                        ctx,
                        code::CANVAS_SIZE,
                        Action::ResizeCanvas {
                            width: size.0,
                            height: size.1,
                        },
                    );
                    self.editor.run_pending();
                    if self.editor.document().canvas == size && size != before {
                        self.remember_canvas_size(size);
                    }
                }
            }
        } else {
            self.confirm = Some(Confirm::Canvas(draft));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resize_dialog_rejects_a_stale_document_revision() {
        for properties in [false, true] {
            let mut app = App::new(None);
            let ctx = egui::Context::default();
            if properties {
                app.begin_image_properties();
            } else {
                app.begin_canvas_dialog(&ctx, false);
            }
            let Some(Confirm::Canvas(draft)) = app.confirm.clone() else {
                panic!("canvas draft");
            };
            app.editor.request(12345, Action::AddLayer);
            app.editor.run_pending();
            let before = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            app.canvas_settings_dialog(&ctx, draft);
            assert!(app.confirm.is_none());
            assert!(app.editor.document().same_semantics(&before));
            assert_eq!(app.editor.history_view().ticket, ticket);
            assert!(app.editor.status().contains("source document changed"));
        }
    }
}
