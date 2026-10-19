use paintcraft_types::Rect;

/// The session's selection projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// No restriction on editing.
    Inactive,
    /// An active selection that covers nothing.
    Empty,
    /// Binary coverage over a half-open document rectangle.
    Rect(Rect),
}
