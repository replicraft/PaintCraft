//! Editor sessions and atomic transaction preparation/publication.
pub mod execute;
pub mod session;

/// Stable editor IDs, geometry, pixels, errors and policy descriptors.
pub use paintcraft_types as types;
