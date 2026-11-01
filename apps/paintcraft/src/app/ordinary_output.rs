//! One bounded private output preparation; UI validates frozen intent before use.
use super::*;
use export_options::{Frozen, Settings};
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetLease};
use paintcraft_document::Document;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Intent {
    Print(String),
    Plan {
        path: PathBuf,
        format: ImageFormat,
        settings: Settings,
        policy: export::metadata_policy::Policy,
    },
    Save {
        path: PathBuf,
        format: ImageFormat,
        settings: Settings,
        policy: export::metadata_policy::Policy,
        losses: Vec<String>,
    },
    Pcx(paintcraft_ui::pcx_export::Encoding),
}
pub(super) enum Output {
    Flat(export::FlatExport),
    Preview(paintcraft_ui::pcx_export::Preview),
    Written(String),
}
/// Owned buffers and captured source are freed before their retention declaration.
struct Owner {
    source: Arc<Document>,
    _lease: BudgetLease,
}
pub(super) struct Packet {
    pub output: Output,
    _owner: Owner,
}
enum Event {
    Ready,
    Complete(Result<Packet>),
}
struct Pending {
    revision: u64,
    frozen: Frozen,
    intent: Intent,
    receiver: mpsc::Receiver<Event>,
    authorize: mpsc::SyncSender<bool>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
struct Preview {
    frozen: Frozen,
    encoding: paintcraft_ui::pcx_export::Encoding,
    packet: Packet,
}
#[derive(Default)]
pub(super) struct State {
    revision: u64,
    pending: Option<Pending>,
    preview: Option<Preview>,
    // The platform runner owns the PNG while the native panel is pending.
    print_hold: Option<Packet>,
}
fn cancelled() -> Error {
    Error::new(
        ErrorCode::OperationCancelled,
        "Output preparation cancelled",
    )
}
fn check(flag: &AtomicBool) -> Result<()> {
    if flag.load(Ordering::Acquire) {
        Err(cancelled())
    } else {
        Ok(())
    }
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Output snapshot charge overflow")
}
fn quote(doc: &Document) -> Result<u64> {
    // Independent native output/source retention policy, not allocator/RSS accounting.
    // Six RGBA planes cover input, conversion and encoded/copy staging; native
    // encoder internal policies remain authoritative. Actual returned capacity
    // must also fit the admitted output allowance before it escapes the worker.
    let pixels = u64::from(doc.canvas.0)
        .checked_mul(u64::from(doc.canvas.1))
        .ok_or_else(limit)?;
    let fixed = (64u64 << 20).checked_add(1 << 20).ok_or_else(limit)?;
    doc.layers
        .values()
        .try_fold(
            fixed
                .checked_add(pixels.checked_mul(24).ok_or_else(limit)?)
                .ok_or_else(limit)?,
            |n, l| {
                n.checked_add(
                    (l.raster.resident_tiles() as u64)
                        .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
                        .ok_or_else(limit)?,
                )
                .and_then(|n| n.checked_add(l.filters.storage_bytes() as u64))
                .and_then(|n| {
                    l.filters
                        .mask_storage_usage()
                        .try_fold(n, |n, (_, b)| n.checked_add(b))
                })
                .and_then(|n| n.checked_add(l.name.len() as u64 + 4096))
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "Output snapshot charge overflow")
                })
            },
        )
        .and_then(|n| {
            n.checked_add(
                doc.source_metadata
                    .as_ref()
                    .map_or(0, |m| m.storage_usage().1),
            )
            .ok_or_else(limit)
        })
}
impl State {
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn start(
        &mut self,
        frozen: Frozen,
        intent: Intent,
        budget: &paintcraft_runtime::SharedBudget,
        ctx: &egui::Context,
    ) -> Result<()> {
        self.revision = self.revision.checked_add(1).ok_or_else(limit)?;
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
            let _ = pending.authorize.try_send(false);
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "An output preparation is already active",
            ));
        }
        let bytes = quote(&frozen.document)?;
        let lease = budget.reserve(bytes)?;
        let source = frozen.document.clone();
        let policy = spatial_workspace::JobPolicy::capture(
            &source,
            None,
            spatial_workspace::needed(&source),
        );
        let work_intent = intent.clone();
        let filter_bake = frozen.filter_bake;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        let (authorize, wait) = mpsc::sync_channel(1);
        let wake = ctx.clone();
        std::thread::Builder::new()
            .name("paintcraft-output-prepare".into())
            .spawn(move || {
                let owner = Owner {
                    source,
                    _lease: lease,
                };
                let source = &owner.source;
                let result = policy.run(|| {
                    let check = || check(&flag);
                    check()?;
                    let output = match &work_intent {
                        Intent::Print(_) => {
                            let rgba = paintcraft_render::render_cancellable(source, check)?;
                            check()?;
                            let bytes = paintcraft_formats::image::encode_checked(
                                ImageFormat::Png,
                                source.canvas.0,
                                source.canvas.1,
                                &rgba,
                                90,
                                &check,
                            )?;
                            check()?;
                            Output::Flat(export::FlatExport {
                                bytes,
                                losses: vec![],
                            })
                        }
                        Intent::Pcx(encoding) => {
                            let rgba = paintcraft_render::render_cancellable(source, check)?;
                            let prepared = paintcraft_formats::pcx::prepare_rgba(
                                source.canvas.0,
                                source.canvas.1,
                                &rgba,
                                export_options::mode(*encoding),
                                &check,
                            )?;
                            let p = prepared.preview(192, &check)?;
                            Output::Preview(paintcraft_ui::pcx_export::Preview {
                                width: p.width,
                                height: p.height,
                                rgba: p.rgba,
                                quantized: prepared.is_quantized(),
                            })
                        }
                        Intent::Plan {
                            format,
                            settings,
                            policy,
                            ..
                        }
                        | Intent::Save {
                            format,
                            settings,
                            policy,
                            ..
                        } => {
                            let mut options = settings.options(*format);
                            options.source_metadata = *policy;
                            options.filter_bake = filter_bake;
                            let all;
                            let losses = match &work_intent {
                                Intent::Save { losses, .. } => losses.as_slice(),
                                _ => {
                                    all = KNOWN_LOSSES
                                        .iter()
                                        .map(|s| (*s).to_owned())
                                        .collect::<Vec<_>>();
                                    &all
                                }
                            };
                            Output::Flat(export::export_flat_options_with_check(
                                source,
                                options,
                                losses,
                                |d| paintcraft_render::render_cancellable(d, check),
                                &check,
                            )?)
                        }
                    };
                    check()?;
                    let capacity = match &output {
                        Output::Flat(f) => f.bytes.capacity(),
                        Output::Preview(p) => p.rgba.capacity(),
                        Output::Written(_) => 0,
                    } as u64;
                    let output_bound =
                        u64::from(source.canvas.0) * u64::from(source.canvas.1) * 24 + (1 << 20);
                    if capacity > output_bound {
                        return Err(Error::new(
                            ErrorCode::ResourceLimit,
                            "Encoded output capacity exceeds its admitted declaration",
                        ));
                    }
                    if let Intent::Save { path, .. } = &work_intent {
                        sender.send(Event::Ready).map_err(|_| cancelled())?;
                        wake.request_repaint();
                        loop {
                            check()?;
                            match wait.recv_timeout(Duration::from_millis(25)) {
                                Ok(true) => break,
                                Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    return Err(cancelled());
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                            }
                        }
                        let Output::Flat(flat) = output else {
                            return Err(cancelled());
                        };
                        check()?;
                        let outcome = save::save_with_preparation(
                            path,
                            true,
                            flat.bytes.len() as u64,
                            |sink| {
                                for block in flat.bytes.chunks(64 * 1024) {
                                    check()?;
                                    sink.write_all(block).map_err(|e| {
                                        Error::new(ErrorCode::StorageFailed, e.to_string())
                                    })?;
                                }
                                Ok(())
                            },
                            |_| check(),
                        );
                        // Prepared bytes are destroyed before the result lease can release.
                        drop(flat);
                        let status = if outcome.is_success() {
                            format!("Exported {}", path.display())
                        } else {
                            format!("Not exported safely: {}", outcome.reasons.join("; "))
                        };
                        Ok(Output::Written(status))
                    } else {
                        Ok(output)
                    }
                });
                let packet = result.map(|output| Packet {
                    output,
                    _owner: owner,
                });
                let _ = sender.send(Event::Complete(packet));
                wake.request_repaint();
            })
            .map_err(|e| Error::new(ErrorCode::ResourceLimit, e.to_string()))?;
        self.pending = Some(Pending {
            revision: self.revision,
            frozen,
            intent,
            receiver,
            authorize,
            cancel,
        });
        Ok(())
    }
}
#[cfg(test)]
#[path = "ordinary_output/tests.rs"]
mod tests;
impl App {
    fn pcx_intent_current(
        &self,
        frozen: &Frozen,
        encoding: paintcraft_ui::pcx_export::Encoding,
    ) -> bool {
        matches!(&self.confirm,Some(Confirm::ExportOptions(draft)) if draft.pcx_matches(frozen,encoding))
    }
    pub(super) fn start_ordinary_output(&mut self, frozen: Frozen, intent: Intent) {
        if !frozen.current(self) {
            return;
        }
        if let Err(error) =
            self.ordinary_output
                .start(frozen, intent, &self.worker_budget, &self.context)
        {
            self.editor.set_status(error.to_string());
        }
        self.context.request_repaint();
    }
    pub(super) fn poll_ordinary_output(&mut self, ctx: &egui::Context) {
        if !self.printer.pending() {
            self.ordinary_output.print_hold = None;
        }
        if self.ordinary_output.preview.as_ref().is_some_and(|p| {
            !p.frozen.current(self) || !self.pcx_intent_current(&p.frozen, p.encoding)
        }) {
            self.ordinary_output.preview = None;
        }
        let Some(pending) = self.ordinary_output.pending.take() else {
            return;
        };
        let current = pending.revision == self.ordinary_output.revision
            && pending.frozen.current(self)
            && match &pending.intent {
                Intent::Pcx(encoding) => self.pcx_intent_current(&pending.frozen, *encoding),
                _ => true,
            };
        if !current {
            pending.cancel.store(true, Ordering::Release);
            let _ = pending.authorize.try_send(false);
        }
        match pending.receiver.try_recv() {
            Ok(Event::Ready) => {
                let _ = pending.authorize.try_send(current);
                self.ordinary_output.pending = Some(pending);
            }
            Ok(Event::Complete(result)) => {
                if !current {
                    return;
                }
                match result {
                    Err(error) if error.code == ErrorCode::OperationCancelled => {}
                    Err(error) => self
                        .editor
                        .set_status(format!("Cannot prepare output: {error}")),
                    Ok(mut packet) => match &pending.intent {
                        Intent::Print(title) => {
                            let Output::Flat(flat) = &mut packet.output else {
                                return;
                            };
                            let bytes = std::mem::take(&mut flat.bytes);
                            let wake = ctx.clone();
                            // Platform printing may outlive App; the runner's wake
                            // closure retains an ordered source/lease owner too.
                            let hold = Owner {
                                source: packet._owner.source.clone(),
                                _lease: packet._owner._lease.clone(),
                            };
                            match self.printer.request(bytes, title.clone(), move || {
                                wake.request_repaint();
                                drop(hold);
                            }) {
                                Ok(()) => self.ordinary_output.print_hold = Some(packet),
                                Err(error) => self.editor.set_status(error),
                            }
                        }
                        Intent::Plan {
                            path,
                            format,
                            settings,
                            policy,
                        } => {
                            let Output::Flat(flat) = &packet.output else {
                                return;
                            };
                            self.confirm = Some(Confirm::Export {
                                exists: path.exists(),
                                path: path.clone(),
                                format: *format,
                                losses: flat.losses.clone(),
                                jpeg_quality: settings.jpeg_quality,
                                jpeg_matte: settings.jpeg_matte,
                                pcx_encoding: settings.pcx_encoding,
                                source: self.editor.preview_stamp(),
                                frozen: pending.frozen.clone(),
                                metadata_policy: *policy,
                            });
                        }
                        Intent::Save { .. } => {
                            if let Output::Written(status) = &packet.output {
                                self.editor.set_status(status.clone());
                            }
                        }
                        Intent::Pcx(encoding) => {
                            self.ordinary_output.preview = Some(Preview {
                                frozen: pending.frozen.clone(),
                                encoding: *encoding,
                                packet,
                            });
                        }
                    },
                }
            }
            Err(mpsc::TryRecvError::Empty) => self.ordinary_output.pending = Some(pending),
            Err(mpsc::TryRecvError::Disconnected) => {
                if current {
                    self.editor.set_status("Output preparation worker stopped.");
                }
            }
        }
        if self.ordinary_output.busy() {
            ctx.request_repaint_after(Duration::from_millis(25));
        }
    }
    pub(super) fn noise_pcx_preview(
        &mut self,
        ui: &mut egui::Ui,
        frozen: &Frozen,
        encoding: paintcraft_ui::pcx_export::Encoding,
    ) -> bool {
        let ready = self
            .ordinary_output
            .preview
            .as_ref()
            .is_some_and(|p| p.frozen == *frozen && p.encoding == encoding);
        if ready {
            let Output::Preview(p) = &self.ordinary_output.preview.as_ref().unwrap().packet.output
            else {
                return false;
            };
            return self
                .pcx_preview
                .show(ui, (self.editor.preview_stamp(), encoding), || {
                    Ok(paintcraft_ui::pcx_export::Preview {
                        width: p.width,
                        height: p.height,
                        rgba: p.rgba.clone(),
                        quantized: p.quantized,
                    })
                });
        }
        if !self.ordinary_output.busy() {
            self.start_ordinary_output(frozen.clone(), Intent::Pcx(encoding));
        }
        ui.spinner();
        ui.ctx().request_repaint_after(Duration::from_millis(25));
        false
    }
}
