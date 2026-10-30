//! One persistence policy for sessions owned by CLI commands and MCP.
use paintcraft_document::Document;
use paintcraft_editor::session::Session;
use paintcraft_formats::pcnd;

/// Check newly authored content at publication. Initial input admission remains
/// with the reader/converter; existing snapshots and history stay navigable.
pub(super) fn session(document: Document) -> Session {
    Session::new(document).with_persistent_validator(Some(pcnd::check_compact_metadata_admission))
}
