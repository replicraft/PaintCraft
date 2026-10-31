//! Browser-owned session. Raster algorithms and history remain in shared crates.
mod filters;
mod floating;
mod pen;
mod resource_shape;
mod sampled_brush;
mod shape_mask;
mod text;
use paintcraft_commands::Batch;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, json::Value};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute,
    session::{HistoryRetention, Session},
};
use paintcraft_tools::{Action, Preview, ToolState, commands};
use uuid::Uuid;

pub(crate) const MAX_PIXELS: u64 = 4_194_304;
pub(crate) const MAX_POINTS: usize = 8192;
pub(crate) const WHITE: Rgba = Rgba([255; 4]);

#[derive(Clone, Copy, Debug)]
pub(crate) struct PublicationWitness {
    pub owner: Uuid,
    pub before: paintcraft_editor::session::Ticket,
    pub state: Uuid,
    pub publication: u64,
    pub history: Option<Uuid>,
    pub committed: bool,
}

#[derive(Debug)]
pub(crate) struct Editor {
    pub session: Session,
    pub tools: ToolState,
    pub target: Option<Uuid>,
    fonts: std::cell::RefCell<paintcraft_tools::text::TextRenderer>,
    resource_shape: Option<resource_shape::Captured>,
    pub(crate) adopted_brush: Option<paintcraft_formats::abr::adoption::AdoptedPreset>,
    floating_source: Option<floating::Source>,
    pen_base: Option<paintcraft_editor::session::Ticket>,
    publication_witness: Option<PublicationWitness>,
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        if u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Browser canvas limit is 4 megapixels",
            ));
        }
        let seed = (Uuid::new_v4().as_u128() as u64) & 9_007_199_254_740_991;
        let doc = new_document(width, height, Some(WHITE), seed, POLICY)?;
        Self::from_document(doc)
    }

    pub fn from_document(doc: Document) -> Result<Self> {
        if u64::from(doc.canvas.0) * u64::from(doc.canvas.1) > MAX_PIXELS {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Browser canvas limit is 4 megapixels",
            ));
        }
        let target = doc.roots.last().copied();
        let mut session = Session::new(doc).with_persistent_validator(Some(
            paintcraft_formats::pcnd::check_compact_metadata_admission,
        ));
        session.set_staged_pixel_budget(MAX_PIXELS * 4);
        session.set_history_budget(64 * 1024 * 1024);
        session.set_history_retention(HistoryRetention::TrimOldest);
        let mut tools = ToolState::default();
        tools.text_style = Some(paintcraft_core::types::text::TextOptions::default());
        Ok(Self {
            session,
            target,
            tools,
            fonts: text::fonts(),
            floating_source: None,
            pen_base: None,
            publication_witness: None,
            adopted_brush: None,
            resource_shape: None,
        })
    }

    pub fn batch(&self, operations: Vec<Value>) -> Result<Batch> {
        let ticket = self.session.ticket();
        Batch::from_value(&Value::object([
            (
                "schema",
                Value::str(Batch::schema_for_operations(&operations)),
            ),
            ("atomic", Value::Bool(true)),
            ("policy", Value::str(POLICY)),
            (
                "base",
                Value::object([
                    ("state_id", Value::str(ticket.state_id.to_string())),
                    (
                        "publication_epoch",
                        Value::str(ticket.publication_epoch.to_string()),
                    ),
                ]),
            ),
            ("operations", Value::Array(operations)),
        ]))
    }

    pub(crate) fn take_publication_witness(&mut self) -> Option<PublicationWitness> {
        self.publication_witness.take()
    }

    fn record_publication(
        &mut self,
        before: paintcraft_editor::session::Ticket,
        receipt: &paintcraft_editor::session::Receipt,
    ) {
        self.publication_witness = Some(PublicationWitness {
            owner: self.session.owner_id(),
            before,
            state: receipt.state_id,
            publication: receipt.publication_epoch,
            history: receipt.history_id,
            committed: receipt.committed,
        });
    }

    pub fn commit(&mut self, operations: Vec<Value>) -> Result<()> {
        let before = self.session.ticket();
        let receipt = self.session.apply(&self.batch(operations)?)?;
        self.target = receipt.aliases.values().next().copied().or(self.target);
        self.retarget();
        self.record_publication(before, &receipt);
        Ok(())
    }

    pub fn retarget(&mut self) {
        if self
            .target
            .is_none_or(|id| !self.session.document().layers.contains_key(&id))
        {
            self.target = self.session.document().roots.last().copied();
        }
    }

    pub fn cancel(&mut self) {
        self.cancel_floating();
        self.tools.cancel_gesture();
        self.tools.cancel_text();
    }

    pub fn apply(&mut self, action: Action) -> Result<()> {
        if let Action::Filter(edit) = &action {
            let operation = self.filter_operation(edit)?;
            // Command output aliases identify filters, not a new active layer.
            let target = self.target;
            self.commit(vec![operation])?;
            self.target = target;
            return Ok(());
        }
        if let Action::MoveSelection {
            layer,
            offset,
            clear,
            transparent_key,
        } = action
        {
            return self.commit_selection_move(layer, offset, clear, transparent_key);
        }
        self.cancel_floating();
        if let Action::ResizeCanvas { width, height }
        | Action::ResizeCanvasAnchored { width, height, .. } = &action
        {
            if u64::from(*width) * u64::from(*height) > MAX_PIXELS {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "Browser canvas limit is 4 megapixels",
                ));
            }
            if !(1..=paintcraft_document::CANVAS_SIDE_MAX).contains(width)
                || !(1..=paintcraft_document::CANVAS_SIDE_MAX).contains(height)
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Canvas dimensions are out of range",
                ));
            }
        }
        if matches!(&action,Action::ResizeCanvas{width,height} | Action::ResizeCanvasAnchored{width,height,..} if (*width,*height)==self.session.document().canvas)
        {
            return Ok(());
        }
        if !matches!(&action, Action::Text { .. } | Action::PickColor(_)) {
            self.place_text()?;
        }
        match action {
            Action::SelectPolygon { points } => {
                self.commit(vec![commands::selection_polygon(&points)?])
            }
            Action::InvertSelection => {
                self.tools.cancel_gesture();
                self.commit(vec![commands::invert_selection()])
            }
            Action::PickColor(pick) => {
                if !self.tools.can_apply_color_pick(&pick) {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "color sample cancelled: tool or color slot changed",
                    ));
                }
                let layer =
                    match pick.source {
                        paintcraft_core::types::picker::SampleSource::Image => None,
                        paintcraft_core::types::picker::SampleSource::Layer => {
                            Some(self.session.document().layers.get(&pick.layer).ok_or_else(
                                || {
                                    Error::new(
                                        ErrorCode::ReferenceUnknownId,
                                        "sampling layer is unavailable",
                                    )
                                },
                            )?)
                        }
                    };
                let color = crate::spatial_host::with_document(self.session.document(), || {
                    paintcraft_render::sampling::sample_color(
                        self.session.document(),
                        layer,
                        pick.at,
                        pick.size,
                        || Ok(()),
                    )
                })?;
                if !self.resource_shape_owner(pick.return_resource_shape) {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "The selected custom shape is no longer available",
                    ));
                }
                self.tools.apply_picked_color(&pick, color);
                Ok(())
            }
            Action::TransformImage { transform, clear } => {
                let op = commands::transform_image(
                    self.session.selection(),
                    self.target,
                    transform,
                    clear,
                )?;
                self.tools.cancel_gesture();
                self.commit(vec![op])
            }
            Action::EraseSelection { color } => {
                let ops = commands::erase_selection(self.session.selection(), self.target, color)?;
                if ops.is_empty() {
                    return Ok(());
                }
                self.tools.cancel_gesture();
                self.commit(ops)
            }
            Action::CropToSelection => {
                let operation =
                    commands::crop_to_selection(self.session.document(), self.session.selection())?;
                self.tools.cancel_gesture();
                self.commit(vec![operation])
            }
            Action::Contour(contour) => self.commit(vec![commands::contour(&contour)]),
            action @ Action::Text { .. } => {
                let operations = self.text_operations(&action)?;
                if operations.is_empty() {
                    Ok(())
                } else {
                    self.commit(operations)
                }
            }
            Action::ToolFailure(message) => Err(Error::new(ErrorCode::ResourceLimit, message)),
            Action::SampledStroke(stroke) => self.commit_sampled(stroke),
            Action::PenStroke(stroke) => self.commit_pen(stroke),
            Action::ResourceShape(shape) => self.commit_resource_shape(shape),
            Action::Stroke(stroke) => {
                self.commit(commands::stroke_operations(&stroke, &|| Ok(()))?)
            }
            Action::Shape {
                layer,
                tool,
                from,
                to,
                stroke,
                fill,
            } => self.commit(vec![commands::shape(layer, tool, from, to, stroke, fill)]),
            Action::Fill {
                layer,
                at,
                color,
                tolerance,
            } => self.commit(vec![Value::object([
                ("op", Value::str("raster.flood_fill")),
                ("layer_ref", layer_ref(layer)),
                ("at", point(at)),
                ("color", Value::str(color.to_hex())),
                ("mode", Value::str("source-over")),
                ("tolerance", Value::Int(tolerance.into())),
            ])]),
            Action::Select { corners } => self.commit(vec![match corners {
                None => Value::object([("op", Value::str("selection.clear"))]),
                Some((a, b)) => Value::object([
                    ("op", Value::str("selection.set_rect")),
                    (
                        "rect",
                        Value::object([
                            ("x", Value::Int(a.0.min(b.0))),
                            ("y", Value::Int(a.1.min(b.1))),
                            ("width", Value::Int((a.0 - b.0).abs() + 1)),
                            ("height", Value::Int((a.1 - b.1).abs() + 1)),
                        ]),
                    ),
                ]),
            }]),
            Action::FlattenImage => {
                self.tools.cancel_gesture();
                let receipt = self.session.flatten(|doc, check| {
                    crate::spatial_host::with_document(doc, || {
                        paintcraft_render::render_cancellable(doc, check)
                    })
                })?;
                self.target = receipt.aliases.get("flattened").copied().or(self.target);
                self.retarget();
                Ok(())
            }
            Action::ClearCanvas { color } => {
                self.cancel();
                self.commit(vec![Value::object([
                    ("op", Value::str("canvas.clear")),
                    ("ref", Value::str("background")),
                    ("color", Value::str(color.to_hex())),
                ])])
            }
            Action::ResizeCanvasAnchored {
                width,
                height,
                anchor,
            } => {
                let operations = commands::resize_canvas_anchored(
                    self.session.document(),
                    width,
                    height,
                    anchor,
                );
                self.cancel();
                if operations.is_empty() {
                    Ok(())
                } else {
                    self.commit(operations)
                }
            }
            Action::ResizeCanvas { width, height } => {
                if u64::from(width) * u64::from(height) > MAX_PIXELS {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "Browser canvas limit is 4 megapixels",
                    ));
                }
                let operations = commands::resize_canvas(self.session.document(), width, height);
                self.cancel();
                if operations.is_empty() {
                    Ok(())
                } else {
                    self.commit(operations)
                }
            }
            Action::Undo | Action::Redo => {
                self.cancel();
                if matches!(action, Action::Undo) {
                    self.session.undo()?;
                } else {
                    self.session.redo()?;
                }
                self.retarget();
                Ok(())
            }
            _ => Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "This browser tool is not connected yet",
            )),
        }
    }

    /// Private staging is also used for release; preview never publishes history.
    pub fn display_document(&self) -> Result<Option<Document>> {
        self.display_preview(self.tools.preview())
    }
    pub(crate) fn display_preview(&self, preview: Option<Preview>) -> Result<Option<Document>> {
        let operations = match preview {
            Some(Preview::Filter(edit)) => vec![self.filter_operation(&edit)?],
            Some(Preview::MoveSelection {
                layer,
                offset,
                clear,
                transparent_key,
            }) => {
                return self.preview_selection_move(layer, offset, clear, transparent_key);
            }
            Some(Preview::Contour(contour)) => vec![commands::contour(&contour)],
            Some(Preview::Path(stroke)) => commands::stroke_operations(&stroke, &|| Ok(()))?,
            Some(Preview::SampledStroke(stroke)) => self.sampled_operations(&stroke)?,
            Some(Preview::PenStroke(stroke)) => stroke
                .resolve_captured(self.session.document().canvas_rect(), &|| Ok(()))?
                .operations(&|| Ok(()))?,
            Some(Preview::ResourceShape(shape)) => self.resource_shape_operations(shape)?,
            Some(Preview::Shape {
                layer,
                tool,
                from,
                to,
                stroke,
                fill,
            }) => vec![commands::shape(layer, tool, from, to, stroke, fill)],
            Some(Preview::Text {
                layer,
                style,
                background,
                at,
                text,
                scale,
                color,
            }) => self.text_operations(&Action::Text {
                layer,
                style,
                background,
                at,
                text,
                scale,
                color,
            })?,
            _ => return Ok(None),
        };
        if operations.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            execute::execute_with_budget(
                self.session.document(),
                self.session.selection(),
                &self.batch(operations)?,
                MAX_PIXELS * 4,
            )?
            .doc,
        ))
    }
}

pub(crate) fn layer_ref(id: Uuid) -> Value {
    Value::object([("id", Value::str(id.to_string()))])
}
fn point(p: (i64, i64)) -> Value {
    Value::object([("x", Value::Int(p.0)), ("y", Value::Int(p.1))])
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_tests;

#[cfg(test)]
mod mask_tests;

#[cfg(test)]
pub(crate) mod persistence_tests;
