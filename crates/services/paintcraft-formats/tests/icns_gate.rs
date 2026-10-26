//! Internal codec qualification never silently becomes image Open or Save.
use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
#[test]
fn icns_remains_unavailable_to_frontends_until_selected_entry_consumers_are_qualified() {
    for platform in [Platform::Native, Platform::Web] {
        for frontend in [
            Frontend::Studio,
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            for operation in [
                Operation::Open,
                Operation::ImportConvert,
                Operation::ExportFlat,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
            ] {
                assert!(
                    !registry::admission(Request::new(frontend, operation, Family::Icns, platform))
                        .available()
                );
            }
        }
    }
}
