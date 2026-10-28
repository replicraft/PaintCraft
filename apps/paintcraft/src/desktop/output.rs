//! Present every semantic step, including texture creation and retirement.
use super::*;

impl Desktop {
    pub(super) fn present(&mut self, mut output: egui::FullOutput, event_loop: &ActiveEventLoop) {
        self.input.handle_platform_output_with_event_loop(
            &self.window,
            event_loop,
            output.platform_output,
        );
        let mut screenshots = Vec::new();
        if let Some(viewport) = output.viewport_output.remove(&ROOT) {
            let cancel = viewport
                .commands
                .contains(&egui::ViewportCommand::CancelClose);
            let close = viewport.commands.contains(&egui::ViewportCommand::Close);
            let mut actions = Vec::new();
            egui_winit::process_viewport_commands(
                &self.ctx,
                &mut self.viewport,
                viewport.commands,
                &self.window,
                &mut actions,
            );
            for action in actions {
                match action {
                    egui_winit::ActionRequested::Screenshot(callback) => screenshots.push(callback),
                    egui_winit::ActionRequested::Copy => {
                        self.input.egui_input_mut().events.push(egui::Event::Copy)
                    }
                    egui_winit::ActionRequested::Cut => {
                        self.input.egui_input_mut().events.push(egui::Event::Cut)
                    }
                    egui_winit::ActionRequested::Paste => {
                        if let Some(text) = self.input.clipboard_text() {
                            self.input
                                .egui_input_mut()
                                .events
                                .push(egui::Event::Paste(text));
                        }
                    }
                    egui_winit::ActionRequested::PaintWhileHidden => self.window.request_redraw(),
                }
            }
            if let Some(when) = Instant::now().checked_add(viewport.repaint_delay) {
                self.schedule(when);
            }
            if (self.close_requested || close) && !cancel {
                if std::env::var_os("PAINTCRAFT_NATIVE_TRACE").is_some()
                    && let Some(native) = &self.native
                {
                    let health = native.with_delivery(|health| health);
                    eprintln!(
                        "PaintCraft owned native before close: health={health:?} report={:?}",
                        native.capabilities()
                    );
                }
                event_loop.exit();
            }
            self.close_requested = false;
        }
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        self.painter.paint_and_update_textures(
            ROOT,
            output.pixels_per_point,
            [0.12, 0.12, 0.12, 1.0],
            &primitives,
            &mut output.textures_delta,
            screenshots,
            &self.window,
        );
    }
}
