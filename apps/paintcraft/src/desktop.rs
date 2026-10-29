//! Owned macOS window with native pointer capture and ordinary-input recovery.
//! Physical tablet and canonical keyboard/IME qualification remain separate.
use crate::{
    App,
    owned_tablet::{Host, Progress},
};
use eframe::{egui, egui_wgpu};
use paintcraft_platform::{dialogs::NativeDialogs, tablet};
use paintcraft_ui::t;
use std::{num::NonZeroU32, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
mod output;
#[cfg(test)]
mod tests;

type Failure = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, Failure>;
const ROOT: egui::ViewportId = egui::ViewportId::ROOT;

#[derive(Clone, Copy, Debug)]
enum Wake {
    Capture,
    Repaint(Instant),
}

/// Normal macOS startup owns the input loop; unavailable native capture keeps
/// the same window, document and ordinary framework input alive.
pub fn run(app: App) -> Result<()> {
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut application = Application {
        initial: Some(app),
        window: None,
        proxy: event_loop.create_proxy(),
        error: None,
    };
    event_loop.run_app(&mut application)?;
    // Installation/callbacks and App services retire before the event loop.
    if std::env::var_os("PAINTCRAFT_NATIVE_TRACE").is_some()
        && let Some(window) = &application.window
    {
        eprintln!(
            "PaintCraft owned native snapshot: {:?}",
            window
                .native
                .as_ref()
                .map(tablet::Installation::capabilities)
        );
    }
    application.window.take();
    match application.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

struct Application {
    initial: Option<App>,
    window: Option<Desktop>,
    proxy: EventLoopProxy<Wake>,
    error: Option<Failure>,
}

struct Desktop {
    // Retire monitor callbacks before Host/App, painter and retained window.
    native: Option<tablet::Installation>,
    host: Host,
    input: egui_winit::State,
    painter: egui_wgpu::winit::Painter,
    ctx: egui::Context,
    window: Arc<Window>,
    viewport: egui::ViewportInfo,
    close_requested: bool,
    repaint_at: Option<Instant>,
}

impl Desktop {
    fn new(app: App, event_loop: &ActiveEventLoop, proxy: EventLoopProxy<Wake>) -> Result<Self> {
        let icon = eframe::icon_data::from_png_bytes(include_bytes!(
            "../../../assets/paintcraft-icon.iconset/icon_256x256@2x.png"
        ))?;
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("PaintCraft")
                    .with_inner_size(winit::dpi::LogicalSize::new(1360.0, 900.0))
                    .with_window_icon(Some(winit::window::Icon::from_rgba(
                        icon.rgba,
                        icon.width,
                        icon.height,
                    )?)),
            )?,
        );
        let ctx = egui::Context::default();
        let repaint = proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            if let Some(when) = Instant::now().checked_add(info.delay) {
                let _ = repaint.send_event(Wake::Repaint(when));
            }
        });
        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            ctx.clone(),
            Default::default(),
            false,
            Default::default(),
        ));
        pollster::block_on(painter.set_window(ROOT, Some(window.clone())))?;
        let state = painter
            .render_state()
            .ok_or_else(|| std::io::Error::other("Owned desktop renderer did not initialize"))?;
        let mut input = egui_winit::State::new(
            ctx.clone(),
            ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );
        input.egui_input_mut().focused = window.has_focus();
        let app = app
            .with_native_window_services(&ctx, NativeDialogs::new(window.as_ref()), Some(&state))
            .with_recovery(paintcraft_formats::recovery::default_dir());
        let budget = app.worker_budget.clone();
        let mut host = Host::new(app, 4096, &budget)?;
        let capture = host.capture_endpoint();
        let trace = std::env::var_os("PAINTCRAFT_NATIVE_TRACE").is_some();
        let native = tablet::install_records(
            window.clone(),
            Arc::new(move |record| {
                let result = capture(record);
                if trace && let Err(error) = &result {
                    eprintln!(
                        "PaintCraft native record refused: installation={} ordinal={} kind={:?} error={error:?}",
                        record.installation,
                        record.ordinal,
                        std::mem::discriminant(&record.event)
                    );
                }
                result
            }),
            Arc::new(move || {
                let _ = proxy.send_event(Wake::Capture);
            }),
        );
        let native = match native {
            Ok(native) => {
                host.bind_delivery(native.delivery_check());
                Some(native)
            }
            Err(error) => {
                eprintln!("PaintCraft native input unavailable: {error}");
                restore_pointer_input(&mut host, &ctx);
                None
            }
        };
        let mut viewport = egui::ViewportInfo::default();
        egui_winit::update_viewport_info(&mut viewport, &ctx, &window, true);
        window.request_redraw();
        Ok(Self {
            native,
            host,
            input,
            painter,
            ctx,
            window,
            viewport,
            close_requested: false,
            repaint_at: None,
        })
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        egui_winit::update_viewport_info(&mut self.viewport, &self.ctx, &self.window, false);
        let mut raw = self.input.take_egui_input(&self.window);
        if self.close_requested {
            self.viewport.events.push(egui::ViewportEvent::Close);
        }
        raw.viewports.insert(ROOT, self.viewport.clone());
        self.viewport.events.clear();
        let output = if let Some(native) = &self.native {
            // All ordinary pointer translation is owned by the native source.
            // Keyboard/IME remain explicit unproven framework barriers in Host.
            raw.events.retain(|event| !framework_pointer(event));
            let result = native
                .with_delivery(|health| self.host.snapshot(health))
                .and_then(|_| self.host.step(&self.ctx, raw));
            match result {
                Ok(step) => {
                    if let Some(error) = &step.error {
                        eprintln!("PaintCraft native input: {error}");
                        self.retire_native();
                    } else if step.progress == Progress::Advanced {
                        self.window.request_redraw();
                    }
                    step.output
                }
                Err(error) => {
                    eprintln!("PaintCraft native input: {error}");
                    self.retire_native();
                    let mut recovery = self.input.take_egui_input(&self.window);
                    recovery.viewports.insert(ROOT, self.viewport.clone());
                    if self.close_requested {
                        recovery
                            .viewports
                            .get_mut(&ROOT)
                            .expect("root viewport")
                            .events
                            .push(egui::ViewportEvent::Close);
                    }
                    self.ctx.run_ui(recovery, |ui| self.host.app_mut().ui(ui))
                }
            }
        } else {
            self.ctx.run_ui(raw, |ui| self.host.app_mut().ui(ui))
        };
        self.present(output, event_loop);
        Ok(())
    }

    fn retire_native(&mut self) {
        // Fail installation authority and remove callbacks before retiring
        // exactly owned jobs. Keep the document and ordinary save UI alive.
        if let Some(native) = self.native.take() {
            if std::env::var_os("PAINTCRAFT_NATIVE_TRACE").is_some() {
                eprintln!("PaintCraft native report: {:?}", native.capabilities());
            }
            drop(native);
        }
        restore_pointer_input(&mut self.host, &self.ctx);
    }

    fn schedule(&mut self, when: Instant) {
        self.repaint_at = Some(self.repaint_at.map_or(when, |old| old.min(when)));
    }
}

/// Capture refusal is an input fallback, not application startup failure.
fn restore_pointer_input(host: &mut Host, ctx: &egui::Context) {
    host.retire();
    let app = host.app_mut();
    app.editor.end_native_activation();
    app.tablet.take();
    app.editor.set_status(t!(
        ctx,
        "Native pen input stopped. You can keep using the mouse and save your work."
    ));
    ctx.stop_dragging();
    ctx.input_mut(|input| input.pointer = Default::default());
    ctx.request_repaint();
}

fn framework_pointer(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::PointerMoved(_)
            | egui::Event::PointerButton { .. }
            | egui::Event::PointerGone
            | egui::Event::MouseMoved(_)
            | egui::Event::MouseWheel { .. }
    )
}

impl ApplicationHandler<Wake> for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let Some(app) = self.initial.take() else {
                return;
            };
            match Desktop::new(app, event_loop, self.proxy.clone()) {
                Ok(window) => self.window = Some(window),
                Err(error) => {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
        } else if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: Wake) {
        if let Some(window) = &mut self.window {
            match event {
                Wake::Capture => window.window.request_redraw(),
                Wake::Repaint(when) => window.schedule(when),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = &mut self.window else {
            return;
        };
        if id != window.window.id() {
            return;
        }
        if matches!(event, WindowEvent::RedrawRequested) {
            if let Err(error) = window.redraw(event_loop) {
                self.error = Some(error);
                event_loop.exit();
            }
            return;
        }
        if let WindowEvent::Resized(size) = event
            && let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            window.painter.on_window_resized(ROOT, width, height);
        }
        if matches!(event, WindowEvent::CloseRequested) {
            window.close_requested = true;
        }
        if matches!(event, WindowEvent::Destroyed) {
            window.native.take();
            event_loop.exit();
            return;
        }
        let response = window.input.on_window_event(&window.window, &event);
        if response.repaint {
            window.window.request_redraw();
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &mut self.window {
            if let Some(native) = &window.native {
                native.cancel();
            }
            window.host.app_mut().retire_owned_tablet();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = &mut self.window else {
            return;
        };
        if let Some(when) = window.repaint_at {
            if when <= Instant::now() {
                window.repaint_at = None;
                window.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(when));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
