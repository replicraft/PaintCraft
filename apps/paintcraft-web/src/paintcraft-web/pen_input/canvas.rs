use super::*;
impl WebApp {
    pub(crate) fn pen_sample_sequence(&self, index: usize) -> Option<u64> {
        self.pen
            .retained
            .as_ref()?
            .frame
            .samples
            .get(index)
            .map(|s| s.sequence)
    }
    pub(crate) fn pen_predecessor_owner(&self) -> Owner {
        self.pen_owner()
    }
    pub(crate) fn pen_contact_predecessor(
        &mut self,
        before: Owner,
        sequence: u64,
        receipt: Option<crate::editor::PublicationWitness>,
        ctx: &egui::Context,
    ) {
        if !self.pen.retained.as_ref().is_some_and(|batch| {
            batch
                .frame
                .samples
                .iter()
                .any(|sample| sample.phase == Phase::Down && sample.sequence > sequence)
        }) {
            return;
        }
        let Some(receipt) = receipt else { return };
        let after = self.pen_owner();
        if !receipt.committed
            || receipt.history.is_none()
            || receipt.owner.as_u128() != before.session
            || receipt.before.state_id.as_u128() != before.state
            || receipt.before.publication_epoch != before.publication
            || receipt.state.as_u128() != after.state
            || receipt.publication != after.publication
        {
            return;
        }
        let Some(transform) = self.pen.previous_transform else {
            return;
        };
        let Some((canvas, _)) = self.pen.region else {
            return;
        };
        let proof = paintcraft_ui::pen_input::Predecessor {
            before,
            after,
            sequence,
            transform,
            canvas,
            pixels_per_point: f64::from(ctx.pixels_per_point()),
            focused: ctx.input(|i| i.focused),
            modal: self.modal_blocked() || self.paste.is_placing(),
            ime: self.text.widget_owned_input,
        };
        if let (Some(source), Some(batch)) = (&self.pen.source, &mut self.pen.retained) {
            // The Source checks an actual later admitted Down as well as the
            // exact previous full owner, view, epoch and one publication step.
            if source.acknowledge_predecessor(batch, proof).is_err() {
                self.retire_pen_host(ctx);
                self.pen.replay_failed = true;
            }
        }
    }
    pub(crate) fn pen_canvas_owned(&self) -> bool {
        self.pen.controller_owned
            || self
                .pen
                .pending
                .as_ref()
                .is_some_and(|(_, b)| !b.frame.samples.is_empty())
    }
    pub(crate) fn pen_canvas_events(
        &mut self,
        events: Vec<Event>,
        ctx: &egui::Context,
    ) -> CanvasFrame {
        let suppress = self.pen.pass.suppress_legacy_pointer();
        let framework = events
            .iter()
            .filter(|e| !suppress || !is_pointer(e))
            .count();
        let pending = self.pen.pending.take();
        let accept = pending.as_ref().is_some_and(|(captured, batch)| {
            self.pen
                .source
                .as_ref()
                .is_some_and(|source| source.batch_is_current(batch, self.pen_owner()))
                && ((batch.frame.samples.is_empty() && !self.pen.controller_owned)
                    || (captured.owner == self.pen_owner()
                        && captured.ppp == f64::from(ctx.pixels_per_point())
                        && !self.modal_blocked()
                        && !self.text.widget_owned_input
                        && !self.paste.is_placing()
                        && (self.pen.controller_owned || captured.config == self.pen_config())
                        && self.canvas.rect.is_some_and(|rect| {
                            batch.frame.samples.iter().all(|sample| {
                                sample.transform.origin
                                    == (f64::from(rect.min.x), f64::from(rect.min.y))
                                    && sample.transform.points_per_pixel == f64::from(self.zoom)
                            })
                        })))
        });
        let samples = pending
            .as_ref()
            .filter(|_| accept)
            .map_or(0, |(_, b)| b.frame.samples.len());
        let mouse = if suppress {
            self.pen.mouse_events.len()
        } else {
            0
        };
        let mut out =
            match CanvasFrame::new(framework + mouse + samples, framework, &self.pen.budget) {
                Ok(out) => out,
                Err(error) => {
                    self.retire_pen_host(ctx);
                    self.pen_failure(ctx, error);
                    return CanvasFrame::empty(&self.pen.budget);
                }
            };
        out.extend(
            events
                .into_iter()
                .filter(|e| !suppress || !is_pointer(e))
                .map(CanvasEvent::Legacy),
        );
        if suppress {
            let (mut mouse, mut pen) = (0, 0);
            loop {
                let m = self.pen.mouse_events.get(mouse).map(|r| r.order);
                let p = pending
                    .as_ref()
                    .filter(|_| accept)
                    .and_then(|(_, b)| b.frame.samples.get(pen))
                    .map(|s| s.sequence);
                if m.is_some() && (p.is_none() || m <= p) {
                    out.push(CanvasEvent::Mouse {
                        sequence: self.pen.mouse_events[mouse].order,
                        event: self.pen.mouse_events[mouse].event.clone(),
                    });
                    mouse += 1;
                } else if p.is_some() {
                    out.push(CanvasEvent::Pen(pen));
                    pen += 1;
                } else {
                    break;
                }
            }
        }
        if let Some((captured, batch)) = pending {
            if !accept {
                self.retire_pen_host(ctx);
            } else {
                self.pen.retained = Some(batch);
                self.pen.retained_config = Some(captured.config);
            }
        }
        out
    }
    pub(crate) fn pen_canvas_sample(&mut self, index: usize, ctx: &egui::Context) -> Option<Event> {
        if self.pen.replay_failed {
            return None;
        }
        let batch = self.pen.retained.as_ref()?;
        if !self
            .pen
            .source
            .as_ref()?
            .batch_is_current(batch, self.pen_owner())
            || self.modal_blocked()
            || self.paste.is_placing()
            || self.text.widget_owned_input
            || !ctx.input(|i| i.focused)
            || f64::from(ctx.pixels_per_point()) != self.pen.ppp
            || self.canvas.rect.is_none_or(|rect| {
                batch.frame.samples.iter().any(|s| {
                    s.transform.origin != (f64::from(rect.min.x), f64::from(rect.min.y))
                        || s.transform.points_per_pixel != f64::from(self.zoom)
                })
            })
            || (!self.pen.controller_owned && self.pen.retained_config != Some(self.pen_config()))
        {
            self.retire_pen_host(ctx);
            self.pen.replay_failed = true;
            return None;
        }
        let sample = *batch.frame.samples.get(index)?;
        if self.pen.fallback_button.is_some()
            || (sample.phase == Phase::Down && !self.editor.tools.pressure_eligible(sample))
        {
            self.pen.controller_owned = sample.phase != Phase::Up;
            self.pen.active_owner = (sample.phase != Phase::Up).then(|| self.pen_owner());
            return legacy_event(sample, &mut self.pen.fallback_button);
        }
        let before = self.pen_owner();
        self.editor.take_publication_witness();
        let result = self.editor.pen_sample(sample, &self.pen.budget);
        let receipt = self.editor.take_publication_witness();
        match result {
            Ok(true) => {
                self.pen.controller_owned = sample.phase != Phase::Up;
                self.pen.active_owner = (sample.phase != Phase::Up).then(|| self.pen_owner());
                self.canvas.invalidate();
                ctx.request_repaint();
                if sample.phase == Phase::Up {
                    self.pen_contact_predecessor(before, sample.sequence, receipt, ctx);
                }
            }
            Ok(false) => return legacy_event(sample, &mut self.pen.fallback_button),
            Err(error) => {
                self.retire_pen_host(ctx);
                self.pen.replay_failed = true;
                self.pen_failure(ctx, error);
            }
        }
        None
    }
    /// Complete layout includes floating windows, dialogs and late UI mutations.
    pub(crate) fn finish_pen_view(&mut self, ctx: &egui::Context) {
        if ctx.will_discard() {
            return;
        }
        let skipped = self
            .pen
            .pending
            .as_ref()
            .is_some_and(|(_, b)| !b.frame.samples.is_empty());
        if skipped
            || (self.pen.controller_owned
                && (self.modal_blocked()
                    || self.pen.active_owner.is_some_and(|o| o != self.pen_owner())))
        {
            self.retire_pen_host(ctx);
        }
        let Some(source) = self.pen.source.as_ref() else {
            return;
        };
        let Some((canvas, canvas_layer)) = self.pen.region else {
            return;
        };
        let Some(rect) = self.canvas.rect else { return };
        let mut transform = Transform {
            owner: self.pen_owner().session,
            epoch: self.pen.epoch,
            origin: (f64::from(rect.min.x), f64::from(rect.min.y)),
            points_per_pixel: f64::from(self.zoom),
        };
        if self.pen.previous_transform.is_some_and(|old| {
            old.origin != transform.origin || old.points_per_pixel != transform.points_per_pixel
        }) {
            let Some(epoch) = self.pen.epoch.checked_add(1) else {
                source.fault();
                return;
            };
            self.pen.epoch = epoch;
            transform.epoch = epoch;
        }
        self.pen.previous_transform = Some(transform);
        self.pen.ppp = f64::from(ctx.pixels_per_point());
        let mut exclusions = [None; 64];
        let mut count = 0;
        if let Some(rect) = self.canvas.controls {
            exclusions[count] = Some(rect);
            count += 1;
        }
        let (layers, overflow, found) = ctx.memory(|m| {
            let mut layers = [None; 64];
            let mut count = 0;
            let mut found = false;
            let mut overflow = false;
            for layer in m.layer_ids() {
                if layer == canvas_layer {
                    found = true;
                    continue;
                }
                if found && m.areas().is_visible(&layer) {
                    if count == layers.len() {
                        overflow = true;
                        break;
                    }
                    layers[count] = Some(layer);
                    count += 1;
                }
            }
            (layers, overflow, found)
        });
        let mut blocked =
            !found || overflow || self.modal_blocked() || egui::Popup::is_any_open(ctx);
        for layer in layers.into_iter().flatten() {
            if let Some(area) =
                egui::containers::AreaState::load(ctx, layer.id).filter(|a| a.interactable)
            {
                if count == exclusions.len() {
                    blocked = true;
                    break;
                }
                exclusions[count] =
                    Some(ctx.layer_transform_to_global(layer).unwrap_or_default() * area.rect());
                count += 1;
            }
        }
        let _ = source.publish(View {
            owner: self.pen_owner(),
            transform,
            pixels_per_point: self.pen.ppp,
            canvas,
            exclusions,
            focused: ctx.input(|i| i.focused),
            blocked,
            ime: self.text.widget_owned_input,
        });
    }
}
