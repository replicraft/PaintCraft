//! Explicit, offscreen screenshots of the real editor, using an edited project.
use super::*;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_core::types::brush::BrushMedia;
use paintcraft_tools::{Action, BlendMode, Contour, Tool};

#[test]
#[ignore = "Explicit offscreen README screenshot export"]
fn export_readme_screenshots() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-readme-screenshots"));
    std::fs::create_dir_all(&output).unwrap();
    let dir = scratch("readme-scenes");
    let path = dir.join("Mountain study.pcnd");
    let mut editor = paintcraft::controller::Editor::new_document(1000, 486).unwrap();
    let layer = editor.target().unwrap();
    let shape = |tool, from, to, color| Action::Shape {
        layer,
        tool,
        from,
        to,
        stroke: None,
        fill: Some(Rgba(color)),
    };
    let contour = |points, color| {
        Action::Contour(Contour {
            layer,
            points,
            closed: true,
            stroke: None,
            fill: Some(Rgba(color)),
        })
    };
    let mut edits = vec![
        shape(Tool::Rect, (0, 0), (999, 485), [216, 237, 249, 255]),
        shape(Tool::Ellipse, (740, 55), (850, 165), [255, 206, 103, 255]),
        shape(Tool::Ellipse, (170, 100), (270, 140), [250, 253, 255, 255]),
        shape(Tool::Ellipse, (235, 80), (320, 140), [250, 253, 255, 255]),
        shape(Tool::Ellipse, (290, 107), (370, 142), [250, 253, 255, 255]),
        contour(vec![(0, 340), (220, 170), (430, 360)], [119, 160, 181, 255]),
        contour(
            vec![(260, 360), (550, 118), (830, 360)],
            [76, 121, 152, 255],
        ),
        contour(
            vec![
                (476, 180),
                (550, 118),
                (620, 179),
                (570, 167),
                (540, 191),
                (510, 167),
            ],
            [248, 252, 255, 255],
        ),
        contour(
            vec![(650, 360), (855, 208), (999, 340), (999, 485)],
            [119, 160, 181, 255],
        ),
        contour(
            vec![
                (0, 321),
                (160, 295),
                (370, 358),
                (650, 308),
                (999, 345),
                (999, 485),
                (0, 485),
            ],
            [116, 170, 135, 255],
        ),
        contour(
            vec![
                (0, 401),
                (285, 346),
                (470, 377),
                (710, 346),
                (999, 421),
                (999, 485),
                (0, 485),
            ],
            [63, 128, 112, 255],
        ),
        contour(
            vec![
                (475, 356),
                (525, 356),
                (561, 400),
                (680, 485),
                (333, 485),
                (465, 408),
            ],
            [148, 205, 222, 255],
        ),
    ];
    for (x, y, size) in [
        (90, 315, 58),
        (155, 328, 46),
        (810, 352, 55),
        (897, 369, 64),
    ] {
        edits.push(shape(
            Tool::Rect,
            (x - 4, y),
            (x + 4, y + size),
            [70, 91, 74, 255],
        ));
        edits.push(contour(
            vec![
                (x, y - size),
                (x - size / 2, y + 12),
                (x + size / 2, y + 12),
            ],
            [43, 101, 88, 255],
        ));
    }
    edits.push(Action::Stroke(paintcraft_tools::Stroke {
        layer,
        points: vec![(85, 451), (113, 440), (145, 450), (182, 438), (217, 448)],
        point_scale: 1,
        color: Rgba([173, 202, 141, 255]),
        size: 10,
        round: true,
        media: BrushMedia::Oil,
        mode: BlendMode::SourceOver,
    }));
    for (i, action) in edits.into_iter().enumerate() {
        assert!(editor.request(i as u64 + 1, action));
        editor.run_pending();
    }
    std::fs::write(
        &path,
        paintcraft_formats::pcnd::write(editor.document()).unwrap(),
    )
    .unwrap();
    for (language, variant, name) in [
        (
            paintcraft_ui::i18n::Language::English,
            ClassicVariant::V11,
            "classic-windows-11",
        ),
        (
            paintcraft_ui::i18n::Language::Japanese,
            ClassicVariant::V11,
            "classic-japanese",
        ),
        (
            paintcraft_ui::i18n::Language::English,
            ClassicVariant::V10,
            "classic-windows-10",
        ),
        (
            paintcraft_ui::i18n::Language::English,
            ClassicVariant::V7,
            "classic-windows-7",
        ),
    ] {
        Settings {
            appearance: if variant == ClassicVariant::V11 {
                Appearance::Dark
            } else {
                Appearance::Light
            },
            language,
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(
            App::new(Some(path.clone())).with_settings(dir.clone()),
            1.0,
            false,
        );
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{name}.png")))
            .unwrap();
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Explicit export of real editor brush strokes for reference comparison"]
fn export_brush_samples() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-readme-screenshots"));
    std::fs::create_dir_all(&output).unwrap();
    let mut editor = paintcraft::controller::Editor::new_document(630, 280).unwrap();
    let layer = editor.target().unwrap();
    for (i, media) in BrushMedia::ALL.into_iter().enumerate() {
        let x = 35 + i as i64 * 70;
        editor.request(
            i as u64 * 2 + 1,
            Action::Text {
                style: None,
                background: Rgba::TRANSPARENT,
                layer,
                at: (x - 4, 14),
                text: (i + 1).to_string(),
                scale: 2,
                color: Rgba([0, 0, 0, 255]),
            },
        );
        editor.run_pending();
        editor.request(
            i as u64 * 2 + 2,
            Action::Stroke(paintcraft_tools::Stroke {
                layer,
                points: vec![(x, 50), (x - 2, 100), (x + 3, 150), (x, 240)],
                point_scale: 1,
                color: Rgba([0, 0, 0, 255]),
                size: 32,
                round: true,
                media,
                mode: BlendMode::SourceOver,
            }),
        );
        editor.run_pending();
    }
    let pixels = paintcraft_render::render_flat(editor.document()).unwrap();
    let bytes = paintcraft_formats::image::encode(
        paintcraft_formats::image::ImageFormat::Png,
        630,
        280,
        &pixels,
        90,
    )
    .unwrap();
    std::fs::write(output.join("brush-samples.png"), bytes).unwrap();
}
