//! Active-layer placement is separate from creating a clipboard layer.
use super::*;
use paintcraft_core::types::{
    Rect,
    policy::{BlendMode, COORD_ABS_MAX},
};
use paintcraft_raster::RgbaPastePlan;

impl Session {
    /// Paste onto an existing visible layer, ignoring the previous selection's
    /// clipping. Placement and optional canvas growth publish as one Paste entry.
    pub fn paste_image_onto(
        &mut self,
        draft: &PasteDraft,
        target: Uuid,
        mode: BlendMode,
        key: Option<Rgba>,
    ) -> Result<Receipt> {
        self.paste_onto_with_admission(
            draft,
            target,
            mode,
            key,
            Admission::new(self.staged_pixels),
            &|| Ok(()),
        )
    }
    fn paste_onto_with_admission(
        &mut self,
        draft: &PasteDraft,
        target: Uuid,
        mode: BlendMode,
        key: Option<Rgba>,
        admission: Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Receipt> {
        check()?;
        if self.history_ticket() != draft.source() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Paste source document or selection changed",
            ));
        }
        let layer = self
            .doc
            .layers
            .get(&target)
            .filter(|l| l.visible)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InputSchema,
                    "Paste requires an existing visible layer",
                )
            })?;
        let local = (
            draft.origin().0.checked_sub(layer.origin.0),
            draft.origin().1.checked_sub(layer.origin.1),
        );
        let (Some(x), Some(y)) = local else {
            return Err(extent_error());
        };
        let mut admission = admission.constrain_pixels(self.staged_pixels);
        let plan = RgbaPastePlan::prepare(
            &layer.raster,
            draft.image().clone(),
            (x, y),
            mode,
            key,
            &admission,
            check,
        )?;
        let offset = plan.origin_offset();
        let origin = (
            layer.origin.0.checked_add(offset.0),
            layer.origin.1.checked_add(offset.1),
        );
        let (Some(ox), Some(oy)) = origin else {
            return Err(extent_error());
        };
        let size = plan.dimensions();
        let extent = Rect::from_origin_size(ox, oy, size.0, size.1).ok_or_else(extent_error)?;
        if [extent.x0, extent.y0, extent.x1, extent.y1]
            .iter()
            .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(extent_error());
        }
        let digest = onto_digest(draft, target, mode, key, check)?;
        let raster = plan.replay(&mut admission, check)?;
        check()?;
        let mut doc = (*self.doc).clone();
        doc.canvas = draft.canvas_size();
        let selection = draft
            .bounds()
            .intersect(&doc.canvas_rect())
            .map_or(Selection::Empty, Selection::Rect);
        let layer = doc.layers.get_mut(&target).expect("validated target");
        layer.raster = raster;
        layer.origin = (ox, oy);
        self.commit_staged(
            crate::execute::Staged {
                doc,
                selection,
                aliases: BTreeMap::from([("pasted".into(), target)]),
            },
            &digest,
            Label::Paste,
            check,
        )
    }
}
impl Preparation {
    /// Uses the same owner/publication/resource guards as all prepared edits.
    #[allow(clippy::too_many_arguments)]
    pub fn paste_image_onto(
        mut self,
        draft: &PasteDraft,
        target: Uuid,
        mode: BlendMode,
        key: Option<Rgba>,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let receipt = self
            .session
            .paste_onto_with_admission(draft, target, mode, key, admission, &check)?;
        self.finish_edit(receipt, check)
    }
}
fn onto_digest(
    draft: &PasteDraft,
    target: Uuid,
    mode: BlendMode,
    key: Option<Rgba>,
    check: &dyn Fn() -> Result<()>,
) -> Result<String> {
    let mut image = Sha256::new();
    image.update(b"paintcraft.paste-rgba/v1");
    image.update(draft.image().width().to_le_bytes());
    image.update(draft.image().height().to_le_bytes());
    for bytes in draft.image().rgba().chunks(65536) {
        check()?;
        image.update(bytes);
    }
    let mode = match mode {
        BlendMode::Replace => "replace",
        BlendMode::SourceOver => "source-over",
    };
    let params = Value::object([
        ("operation", Value::str("paintcraft.paste-onto/v1")),
        ("target", Value::str(target.to_string())),
        ("rgba_sha256", Value::str(hex(&image.finalize()))),
        ("mode", Value::str(mode)),
        (
            "rgb_key",
            key.map_or(Value::Null, |key| {
                Value::Array(
                    key.0[..3]
                        .iter()
                        .map(|v| Value::Int(i64::from(*v)))
                        .collect(),
                )
            }),
        ),
        ("x", Value::Int(draft.origin().0)),
        ("y", Value::Int(draft.origin().1)),
        ("canvas_width", Value::Int(i64::from(draft.canvas_size().0))),
        (
            "canvas_height",
            Value::Int(i64::from(draft.canvas_size().1)),
        ),
    ]);
    Ok(hex(&Sha256::digest(
        paintcraft_core::types::json::canonical(&params),
    )))
}
fn extent_error() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Paste target extent exceeds coordinate policy",
    )
}
