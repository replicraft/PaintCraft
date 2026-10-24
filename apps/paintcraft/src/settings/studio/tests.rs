use super::*;
use crate::settings::{Experience, Loaded, Settings};
use paintcraft_core::types::json;

#[test]
fn old_settings_keep_all_studio_panels_visible_and_positionless() {
    for version in ["v1", "v2"] {
        let input =
            format!(r#"{{"schema":"paintcraft.settings/{version}","experience":"studio"}}"#);
        let settings = crate::settings::parse(input.as_bytes()).unwrap();
        assert_eq!(settings.studio_layout, Layout::default());
    }
    assert_eq!(parse(None).unwrap(), Layout::default());
    assert_eq!(
        parse(Some(&Value::Object(Default::default()))).unwrap(),
        Layout::default()
    );
}

#[test]
fn panel_preferences_roundtrip_without_changing_classic_or_other_studio_settings() {
    let directory =
        std::env::temp_dir().join(format!("paintcraft-studio-layout-{}", uuid::Uuid::new_v4()));
    let mut settings = Settings {
        experience: Experience::Classic,
        classic_rulers: true,
        classic_status_bar: false,
        studio_overscroll: false,
        studio_auto_scroll: false,
        ..Default::default()
    };
    for (index, panel) in settings.studio_layout.panels.iter_mut().enumerate() {
        panel.visible = index % 2 == 0;
        panel.position = Some([100 + index as i32 * 30, 200 + index as i32 * 40]);
    }
    settings.save(&directory).unwrap();
    assert_eq!(Settings::load(&directory), (settings, Loaded::Ok));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_layout_is_rejected_and_original_settings_are_preserved() {
    for input in [
        r#"false"#,
        r#"{"tools":false}"#,
        r#"{"tools":{"visible":"false"}}"#,
        r#"{"tools":{"visible":true,"position":[1]}}"#,
        r#"{"tools":{"visible":true,"position":[1,"2"]}}"#,
        r#"{"tools":{"visible":true,"position":[-32769,0]}}"#,
        r#"{"tools":{"visible":true,"position":[0,32769]}}"#,
    ] {
        let source = format!(
            r#"{{"schema":"paintcraft.settings/v2","experience":"studio","studio_layout":{input}}}"#
        );
        let directory =
            std::env::temp_dir().join(format!("paintcraft-bad-layout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("settings.json"), &source).unwrap();
        let (settings, loaded) = Settings::load(&directory);
        assert_eq!(settings, Settings::default());
        let Loaded::Unusable {
            moved_to: Some(path),
            ..
        } = loaded
        else {
            panic!("{loaded:?}");
        };
        assert_eq!(std::fs::read(path).unwrap(), source.as_bytes());
        std::fs::remove_dir_all(directory).unwrap();
    }
    let boundary =
        json::parse(br#"{"history":{"visible":false,"position":[-32768,32768]}}"#).unwrap();
    assert_eq!(
        parse(Some(&boundary)).unwrap().panels[3].position,
        Some([-32768, 32768])
    );
}
