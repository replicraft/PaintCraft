//! Capture immutable native-size tips alongside ordinary pointer ownership.
use super::*;
impl ToolState {
    pub(super) fn sampled_gesture(
        g: &Gesture,
    ) -> paintcraft_core::types::Result<sampled_brush::SampledStroke> {
        let invalid = || {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "sampled gesture admission failed",
            )
        };
        if g.sampled_failure {
            return Err(invalid());
        }
        let (tip, spacing) = g.sampled.as_ref().ok_or_else(invalid)?;
        let color = Self::brush_of(g).2;
        let mut stroke = sampled_brush::SampledStroke::begin(
            g.layer,
            color,
            tip.clone(),
            *g.points.first().ok_or_else(invalid)?,
            g.point_scale,
            *spacing,
        )?;
        for point in &g.points[1..] {
            stroke.append(*point)?;
        }
        if let Some(id) = g.sampled_source {
            stroke = stroke.with_resource_id(id);
        }
        Ok(stroke)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tools() -> ToolState {
        let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
        let tip = paintcraft_raster::sampled_brush::SampledTip::from_alpha(
            2,
            1,
            (1, 0),
            &[128, 255],
            &budget,
            &|| Ok(()),
        )
        .unwrap();
        ToolState {
            tool: Tool::Brush,
            sampled_tip: Some((tip, 256)),
            sampled_source: Some(Uuid::new_v4()),
            ..Default::default()
        }
    }
    #[test]
    fn sampled_prefix_keeps_tip_color_opacity_source_and_button_until_matching_release() {
        let mut tools = tools();
        let id = tools.sampled_source;
        let layer = Uuid::new_v4();
        tools.options.brush_opacity = 50;
        tools.pointer_down_subpixel(layer, (512, 512), PointerButton::Secondary);
        tools.pointer_move_subpixel((768, 512));
        let Some(Preview::SampledStroke(preview)) = tools.preview() else {
            panic!()
        };
        tools.sampled_tip = None;
        tools.sampled_source = Some(Uuid::new_v4());
        tools.options.brush_opacity = 1;
        assert!(tools.pointer_up_button(PointerButton::Primary).is_none());
        let (_, Action::SampledStroke(released)) =
            tools.pointer_up_button(PointerButton::Secondary).unwrap()
        else {
            panic!()
        };
        assert_eq!(preview, released);
        assert_eq!(released.resource_id(), id);
        let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
        let clip = paintcraft_core::types::Rect::new(0, 0, 8, 8).unwrap();
        assert_eq!(
            preview
                .preview(clip, &budget, &|| Ok(()))
                .unwrap()
                .operations(&|| Ok(()))
                .unwrap(),
            released
                .finish(clip, &budget, &|| Ok(()))
                .unwrap()
                .operations(&|| Ok(()))
                .unwrap()
        );
    }
    #[test]
    fn sampled_limit_deduplicates_endpoint_but_rejects_the_entire_overflowing_gesture() {
        let mut tools = tools();
        tools.pointer_down_button(Uuid::new_v4(), (0, 0), PointerButton::Primary);
        let points = &mut tools.gesture.as_mut().unwrap().points;
        points.extend((1..paintcraft_raster::sampled_brush::POINTS_MAX).map(|i| (i as i64, 0)));
        let endpoint = *tools.gesture.as_ref().unwrap().points.last().unwrap();
        tools.pointer_move(endpoint);
        assert!(!tools.gesture.as_ref().unwrap().sampled_failure);
        tools.pointer_move((endpoint.0 + 1, 0));
        assert!(tools.preview().is_none());
        assert!(matches!(
            tools.pointer_up(),
            Some((_, Action::ToolFailure(_)))
        ));
        assert!(!tools.pointer_is_down());
    }
    #[test]
    fn conservative_work_admission_refuses_without_truncating_or_mutating_the_stroke() {
        let mut tools = tools();
        tools.pointer_down_button(Uuid::nil(), (0, 0), PointerButton::Primary);
        tools.pointer_move((100, 0));
        let Some(Preview::SampledStroke(stroke)) = tools.preview() else {
            panic!()
        };
        let before = stroke.clone();
        assert!(stroke.admit_work(10).is_err());
        assert!(stroke.admit_work(10000).is_ok());
        assert_eq!(stroke, before);
    }
}
