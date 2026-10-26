use paintcraft_formats::registry::{
    self, Decision, Family, Frontend, Operation, Platform, Request,
};
#[test]
fn abr_is_only_a_studio_asset_import_on_both_platforms() {
    for platform in [Platform::Native, Platform::Web] {
        let mut request = Request::new(
            Frontend::Studio,
            Operation::ImportAsset,
            Family::Abr,
            platform,
        );
        request.extension = Some("abr");
        assert!(registry::admission(request).available());
        assert!(
            registry::enabled(Frontend::Studio, Operation::ImportAsset, platform)
                .contains(&Family::Abr)
        );
        request.extension = Some("png");
        assert!(!registry::admission(request).available());
        for operation in [
            Operation::Open,
            Operation::OpenProject,
            Operation::ImportConvert,
            Operation::ExportFlat,
            Operation::ExportAsset,
            Operation::SaveAsCopy,
        ] {
            assert!(matches!(
                registry::admission(Request::new(
                    Frontend::Studio,
                    operation,
                    Family::Abr,
                    platform
                )),
                Decision::Denied(_)
            ));
        }
        for frontend in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            assert!(
                !registry::admission(Request::new(
                    frontend,
                    Operation::ImportAsset,
                    Family::Abr,
                    platform
                ))
                .available()
            );
        }
    }
}
