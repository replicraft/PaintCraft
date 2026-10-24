//! Owner-bound selection changes use normal ordered transaction preparation.
use super::*;
impl Editor {
    pub(super) fn select_polygon(&mut self, points: &[(i64, i64)]) {
        match raster_commands::selection_polygon(points) {
            Ok(operation) => {
                self.commit(vec![operation], "Selection changed");
            }
            Err(error) => self.status = error.message,
        }
    }
    pub(super) fn invert_selection(&mut self) {
        self.tools.cancel_gesture();
        self.commit(
            vec![raster_commands::invert_selection()],
            "Selection changed",
        );
    }
}
