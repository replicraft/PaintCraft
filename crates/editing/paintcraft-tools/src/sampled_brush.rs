//! Captured immutable sampled-tip gestures. Preview and release resolve the
//! same coverage and publish only through existing raster.mask_spans commands.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result, Rgba, json::Value, policy::COORD_ABS_MAX,
    resources::SharedBudget,
};
use paintcraft_raster::sampled_brush::{self, Coverage, SampledTip};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct SampledStroke {
    resource_id: Option<Uuid>,
    layer: Uuid,
    color: Rgba,
    tip: SampledTip,
    points: Vec<(i64, i64)>,
    point_scale: u32,
    spacing_q8: u32,
}
#[derive(Clone, Debug)]
pub struct ResolvedSampledStroke {
    layer: Uuid,
    color: Rgba,
    coverage: Coverage,
}
impl ResolvedSampledStroke {
    pub fn coverage(&self) -> &Coverage {
        &self.coverage
    }
    pub fn color(&self) -> Rgba {
        self.color
    }
    pub fn layer(&self) -> Uuid {
        self.layer
    }
    pub fn operations_with_first_layer(
        &self,
        canvas: (u32, u32),
        check: &impl Fn() -> Result<()>,
    ) -> Result<Vec<Value>> {
        crate::commands::first_layer_mask_operations(
            canvas,
            self.color,
            self.coverage.spans(),
            "sampled-first",
            "Layer 1",
            check,
        )
    }

    pub fn operations(&self, check: &impl Fn() -> Result<()>) -> Result<Vec<Value>> {
        crate::commands::mask_operations(self.layer, self.color, self.coverage.spans(), check)
    }
}
impl SampledStroke {
    /// Native-size static tip policy. Captures opacity in `color`; later UI
    /// preferences or asset selection cannot change an active gesture.
    pub fn begin(
        layer: Uuid,
        color: Rgba,
        tip: SampledTip,
        at: (i64, i64),
        point_scale: u32,
        spacing_q8: u32,
    ) -> Result<Self> {
        if !matches!(point_scale, 1 | 256)
            || spacing_q8 == 0
            || spacing_q8 > sampled_brush::SPACING_Q8_MAX
        {
            return Err(invalid("invalid sampled brush scale or spacing"));
        }
        validate_point(at, point_scale)?;
        Ok(Self {
            resource_id: None,
            layer,
            color,
            tip,
            points: vec![at],
            point_scale,
            spacing_q8,
        })
    }
    pub fn with_resource_id(mut self, id: Uuid) -> Self {
        self.resource_id = Some(id);
        self
    }
    pub fn resource_id(&self) -> Option<Uuid> {
        self.resource_id
    }
    pub fn append(&mut self, point: (i64, i64)) -> Result<()> {
        validate_point(point, self.point_scale)?;
        if self.points.last() == Some(&point) {
            return Ok(());
        }
        if self.points.len() == sampled_brush::POINTS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "sampled gesture point allowance exceeded",
            ));
        }
        self.points.try_reserve(1).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot retain sampled gesture points",
            )
        })?;
        self.points.push(point);
        Ok(())
    }
    pub fn admit_work(&self, allowance: u64) -> Result<()> {
        let factor = 256 / u128::from(self.point_scale);
        let length = self
            .points
            .windows(2)
            .try_fold(0u128, |total, pair| {
                total.checked_add(
                    u128::from(pair[0].0.abs_diff(pair[1].0))
                        + u128::from(pair[0].1.abs_diff(pair[1].1)),
                )
            })
            .ok_or_else(|| invalid("sampled work overflow"))?
            * factor;
        let dabs = 1 + length / u128::from(self.spacing_q8);
        let work = dabs * u128::from(self.tip.width() + 2) * u128::from(self.tip.height() + 2);
        if work > u128::from(allowance) {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Sampled brush stroke exceeds the browser work limit. Use shorter strokes or a smaller original tip.",
            ));
        }
        Ok(())
    }
    pub fn tip(&self) -> &SampledTip {
        &self.tip
    }
    pub fn points(&self) -> &[(i64, i64)] {
        &self.points
    }
    pub fn preview(
        &self,
        clip: Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ResolvedSampledStroke> {
        self.resolve(clip, budget, check)
    }
    pub fn finish(
        self,
        clip: Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ResolvedSampledStroke> {
        self.resolve(clip, budget, check)
    }
    /// Cancellation publishes no command and releases this captured owner.
    pub fn cancel(self) {}
    fn resolve(
        &self,
        clip: Rect,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ResolvedSampledStroke> {
        let coverage = if self.color.a() == 0 {
            Coverage::default()
        } else {
            sampled_brush::coverage(
                &self.tip,
                &self.points,
                self.point_scale,
                self.spacing_q8,
                clip,
                budget,
                check,
            )?
        };
        check()?;
        Ok(ResolvedSampledStroke {
            layer: self.layer,
            color: self.color,
            coverage,
        })
    }
}
fn validate_point(point: (i64, i64), scale: u32) -> Result<()> {
    let bound = (COORD_ABS_MAX * i64::from(scale)) as u64;
    if point.0.unsigned_abs() > bound || point.1.unsigned_abs() > bound {
        return Err(invalid("sampled gesture coordinates exceed numeric policy"));
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_and_release_capture_the_same_tip_color_layer_and_mask_commands() {
        let budget = SharedBudget::new(1 << 20);
        let tip = SampledTip::from_alpha(3, 1, (1, 0), &[0, 64, 255], &budget, &|| Ok(())).unwrap();
        let layer = Uuid::nil();
        let color = Rgba([20, 40, 60, 128]);
        let mut gesture = SampledStroke::begin(layer, color, tip.clone(), (2, 2), 1, 256).unwrap();
        gesture.append((4, 2)).unwrap();
        let clip = Rect::new(0, 0, 8, 8).unwrap();
        let preview = gesture.preview(clip, &budget, &|| Ok(())).unwrap();
        assert!(gesture.tip().shares_storage(&tip));
        let released = gesture.finish(clip, &budget, &|| Ok(())).unwrap();
        assert_eq!(preview.coverage().spans(), released.coverage().spans());
        assert_eq!(
            preview.operations(&|| Ok(())).unwrap(),
            released.operations(&|| Ok(())).unwrap()
        );
        assert_eq!(released.color(), color);
        assert_eq!(released.layer(), layer);
        let operations = released.operations(&|| Ok(())).unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(
            operations[0].get("op").and_then(Value::as_str),
            Some("raster.mask_spans")
        );
        assert_eq!(
            operations[0].get("color").and_then(Value::as_str),
            Some("#14283C80")
        );
    }
    #[test]
    fn transparent_and_all_zero_tips_have_no_publishable_operation() {
        let budget = SharedBudget::new(1 << 20);
        let tip = SampledTip::from_alpha(1, 1, (0, 0), &[0], &budget, &|| Ok(())).unwrap();
        let gesture =
            SampledStroke::begin(Uuid::nil(), Rgba([1, 2, 3, 255]), tip, (0, 0), 1, 256).unwrap();
        assert!(
            gesture
                .finish(
                    Rect::new(0, 0, 8, 8).unwrap(),
                    &SharedBudget::new(0),
                    &|| Ok(())
                )
                .unwrap()
                .operations(&|| Ok(()))
                .unwrap()
                .is_empty()
        );
        let tip = SampledTip::from_alpha(1, 1, (0, 0), &[255], &budget, &|| Ok(())).unwrap();
        let gesture =
            SampledStroke::begin(Uuid::nil(), Rgba::TRANSPARENT, tip.clone(), (0, 0), 1, 256)
                .unwrap();
        gesture.cancel();
        let gesture =
            SampledStroke::begin(Uuid::nil(), Rgba::TRANSPARENT, tip, (0, 0), 1, 256).unwrap();
        assert!(
            gesture
                .finish(
                    Rect::new(0, 0, 8, 8).unwrap(),
                    &SharedBudget::new(0),
                    &|| Ok(())
                )
                .unwrap()
                .operations(&|| Ok(()))
                .unwrap()
                .is_empty()
        );
    }
}
