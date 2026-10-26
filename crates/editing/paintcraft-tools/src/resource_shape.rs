//! Immutable resource intentions. Hosts resolve geometry using the captured policy.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, policy::COORD_ABS_MAX};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference {
    pub library: Uuid,
    pub preset: usize,
    pub policy: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub resource: Reference,
    pub layer: Uuid,
    pub from: (i64, i64),
    pub to: (i64, i64),
    pub color: Rgba,
}
impl Placement {
    /// Pixel-inclusive drag endpoints; checked before a host allocates any pixels.
    pub fn target_box(self, side: u32, pixels: u64) -> Result<((i64, i64), u32, u32)> {
        let invalid = || {
            Error::new(
                ErrorCode::ResourceLimit,
                "Custom shape size or position exceeds the drawing limit",
            )
        };
        let at = (self.from.0.min(self.to.0), self.from.1.min(self.to.1));
        let width = self
            .from
            .0
            .abs_diff(self.to.0)
            .checked_add(1)
            .ok_or_else(invalid)?;
        let height = self
            .from
            .1
            .abs_diff(self.to.1)
            .checked_add(1)
            .ok_or_else(invalid)?;
        if width > u64::from(side)
            || height > u64::from(side)
            || width.checked_mul(height).is_none_or(|area| area > pixels)
            || [
                Some(at.0),
                Some(at.1),
                at.0.checked_add_unsigned(width),
                at.1.checked_add_unsigned(height),
            ]
            .iter()
            .any(|v| v.is_none_or(|v| v.unsigned_abs() > COORD_ABS_MAX as u64))
        {
            return Err(invalid());
        }
        Ok((at, width as u32, height as u32))
    }
}
impl ToolState {
    /// Resource adoption is explicit. Selecting any stock tool clears this choice.
    pub fn select_resource_shape(&mut self, resource: Reference) {
        self.cancel_gesture();
        self.choose_tool(Tool::Rect);
        self.resource_shape = Some(resource);
        self.preview_revision.advance();
    }
    pub(super) fn resource_shape_gesture(g: &Gesture) -> Placement {
        let (from, to) = geometry::endpoints(g.tool, &g.points, g.constrain_shape);
        Placement {
            resource: g.resource_shape.expect("a captured resource shape"),
            layer: g.layer,
            from,
            to,
            color: g.color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(preset: usize) -> Reference {
        Reference {
            library: Uuid::from_u128(7),
            preset,
            policy: "test-filled-cubic/v1",
        }
    }
    fn preview(tools: &ToolState) -> Placement {
        let Some(Preview::ResourceShape(placement)) = tools.preview() else {
            panic!("resource preview")
        };
        placement
    }
    #[test]
    fn gesture_captures_resource_color_target_and_preview_release_exactly() {
        let mut tools = ToolState::default();
        tools.select_resource_shape(reference(3));
        tools.color = Rgba([9, 8, 7, 61]);
        let layer = Uuid::from_u128(12);
        tools.pointer_down(layer, (20, 30));
        tools.pointer_move((4, 12));
        tools.resource_shape = Some(reference(4));
        tools.color = Rgba([255; 4]);
        let shown = preview(&tools);
        assert_eq!(shown.resource, reference(3));
        assert_eq!(shown.layer, layer);
        assert_eq!(shown.color, Rgba([9, 8, 7, 61]));
        assert_eq!(shown.target_box(4096, 262_144).unwrap(), ((4, 12), 17, 19));
        let Some((_, Action::ResourceShape(released))) = tools.pointer_up() else {
            panic!("resource action")
        };
        assert_eq!(released, shown);
        assert!(tools.preview().is_none());
    }
    #[test]
    fn shift_and_subpixel_reuse_rectangle_endpoints_without_losing_raw_geometry() {
        let mut tools = ToolState::default();
        tools.select_resource_shape(reference(0));
        tools.secondary = Rgba([4, 5, 6, 100]);
        tools.pointer_down_subpixel(
            Uuid::nil(),
            (10 * 256 + 100, 12 * 256 + 100),
            PointerButton::Secondary,
        );
        tools.pointer_move_subpixel((5 * 256 + 240, 10 * 256 + 100));
        assert_eq!(
            preview(&tools).target_box(4096, 262_144).unwrap(),
            ((5, 10), 6, 3)
        );
        tools.constrain_shape(true);
        let square = preview(&tools);
        assert_eq!(square.target_box(4096, 262_144).unwrap(), ((5, 7), 6, 6));
        assert_eq!(square.color, tools.secondary);
        tools.constrain_shape(false);
        assert_eq!(preview(&tools).to, (5, 10));
        tools.constrain_shape(true);
        let Some((_, Action::ResourceShape(released))) =
            tools.pointer_up_button(PointerButton::Secondary)
        else {
            panic!("secondary release")
        };
        assert_eq!(released, square);
    }
    #[test]
    fn stock_reselection_cancellation_and_invalid_extents_never_guess_geometry() {
        let mut tools = ToolState::default();
        tools.select_resource_shape(reference(0));
        tools.pointer_down(Uuid::nil(), (-1, -1));
        let click = preview(&tools);
        assert_eq!(click.target_box(4096, 262_144).unwrap(), ((-1, -1), 1, 1));
        tools.cancel_gesture();
        assert!(tools.pointer_up().is_none());
        tools.choose_tool(Tool::Rect);
        assert!(tools.resource_shape.is_none());
        tools.pointer_down(Uuid::nil(), (0, 0));
        assert!(matches!(tools.preview(), Some(Preview::Shape { .. })));
        for (from, to) in [
            ((0, 0), (512, 512)),
            ((i64::MIN, 0), (i64::MAX, 0)),
            ((COORD_ABS_MAX, 0), (COORD_ABS_MAX, 0)),
        ] {
            assert!(
                Placement { from, to, ..click }
                    .target_box(4096, 262_144)
                    .is_err()
            );
        }
        assert!(
            Placement {
                from: (0, 0),
                to: (511, 511),
                ..click
            }
            .target_box(4096, 262_144)
            .is_ok()
        );
        tools.select_resource_shape(reference(0));
        tools.activate_tool(Tool::Pencil);
        assert!(tools.resource_shape.is_none());
    }
}
