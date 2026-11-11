use super::*;
use egui::{Id, Rect, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::{Rgba, brush::BrushMedia};
use paintcraft_tools::{SelectionMode, Tool, ToolOptions};
use paintcraft_ui::{
    actions::{Action, Context, Frontend},
    chrome::Controls,
};

fn context(blocked: bool) -> Context {
    Context {
        frontend: Frontend::Retro,
        document_open: true,
        can_undo: true,
        can_redo: true,
        can_recent: false,
        can_paste: true,
        can_paste_onto: true,
        can_save_all: false,
        selection_active: true,
        tool: Tool::Pencil,
        blocked,
        layer_index: Some(0),
        layer_count: 1,
        can_flatten: false,
        layer_visible: true,
        zoom: 1.0,
        pixel_grid: false,
        can_fit_selection: true,
        can_crop_selection: true,
        can_erase_selection: true,
        can_transform_image: true,
        can_rotate_custom: false,
        status_bar: true,
        rulers: false,
        fullscreen: false,
        can_exit: false,
        desktop_download: false,
        auto_hide_toolbar: false,
        thumbnail: false,
    }
}
struct State {
    controls: Controls,
    workspace: Workspace,
    intents: Vec<Intent>,
    initialized: bool,
    blocked: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            controls: Controls {
                tool: Tool::Pencil,
                options: ToolOptions::default(),
                primary: Rgba([0, 0, 0, 255]),
                secondary: Rgba([255; 4]),
            },
            workspace: Workspace::default(),
            intents: Vec::new(),
            initialized: false,
            blocked: false,
        }
    }
}
fn initialize(ui: &mut egui::Ui, state: &mut State) {
    if !state.initialized {
        paintcraft_ui::fonts::install(ui.ctx());
        state.initialized = true;
    }
}

#[test]
fn all_sixteen_tool_choices_have_distinct_effective_selection_and_real_options() {
    let mut controls = State::default().controls;
    for choice in ToolChoice::ALL {
        choice.apply_to(&mut controls);
        assert_eq!(controls.tool, choice.tool());
        assert!(choice.matches(&controls));
        assert_eq!(
            ToolChoice::ALL
                .into_iter()
                .filter(|candidate| candidate.matches(&controls))
                .count(),
            1
        );
    }
    ToolChoice::FreeFormSelect.apply_to(&mut controls);
    assert_eq!(controls.options.selection_mode, SelectionMode::FreeForm);
    ToolChoice::Select.apply_to(&mut controls);
    assert_eq!(controls.options.selection_mode, SelectionMode::Rectangle);
    ToolChoice::Airbrush.apply_to(&mut controls);
    assert_eq!(controls.options.media, BrushMedia::Airbrush);
    assert!(controls.options.round);
    ToolChoice::Brush.apply_to(&mut controls);
    assert_eq!(controls.options.media, BrushMedia::Plain);
}

#[test]
fn actual_tool_buttons_only_emit_intentions_and_blocked_controls_do_not_execute() {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            initialize(ui, state);
            state.intents.extend(toolbox(
                ui,
                &mut state.controls,
                &mut state.workspace,
                context(state.blocked),
            ));
        },
        State::default(),
    );
    h.run();
    for choice in ToolChoice::ALL {
        h.state_mut().intents.clear();
        h.get_by_label(choice.label()).click();
        h.run();
        assert_eq!(h.state().intents, [Intent::ChooseTool(choice)]);
        assert_eq!(
            h.state().controls.tool,
            Tool::Pencil,
            "widget must not accept host transition"
        );
        assert_eq!(h.state().controls.options.media, BrushMedia::Plain);
        assert_eq!(
            h.state().controls.options.selection_mode,
            SelectionMode::Rectangle
        );
    }
    h.state_mut().blocked = true;
    h.state_mut().intents.clear();
    h.run();
    assert!(h.get_by_label("Pencil").accesskit_node().is_disabled());
    h.get_by_label("Pencil").click();
    h.run();
    assert!(h.state().intents.is_empty());
}

#[test]
fn actual_palette_primary_secondary_and_accessible_secondary_target_change_real_controls() {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            initialize(ui, state);
            state.intents.extend(palette(
                ui,
                &mut state.controls,
                &mut state.workspace,
                context(false),
            ));
        },
        State::default(),
    );
    h.run();
    h.get_by_label("Palette color #FF0000").click();
    h.run();
    assert_eq!(h.state().controls.primary, Rgba([255, 0, 0, 255]));
    h.get_by_label("Palette color #0000FF").click_secondary();
    h.run();
    assert_eq!(h.state().controls.secondary, Rgba([0, 0, 255, 255]));
    h.get_by_label("Color 2").click();
    h.run();
    assert!(h.state().workspace.edit_secondary);
    h.get_by_label("Palette color #00FF00").click();
    h.run();
    assert_eq!(h.state().controls.secondary, Rgba([0, 255, 0, 255]));
    assert_eq!(h.state().controls.primary, Rgba([255, 0, 0, 255]));
}

#[test]
fn real_view_menu_can_hide_and_restore_panels_and_browser_file_menu_has_no_exit() {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            initialize(ui, state);
            state
                .intents
                .extend(menu_bar(ui, &mut state.workspace, context(false)));
        },
        State::default(),
    );
    h.run();
    for expected in [false, true] {
        h.get_by_label("View").click();
        h.run();
        h.get_by_label("Tools").click();
        h.run();
        assert_eq!(h.state().workspace.toolbox, expected);
        egui::Popup::close_all(&h.ctx);
        h.run();
    }
    h.get_by_label("File").click();
    h.run();
    assert!(h.query_by_label("Exit").is_none());
    let shortcut = Action::Save.descriptor().shortcut.unwrap();
    h.get_by_label(&format!("Save {}", h.ctx.format_shortcut(&shortcut)))
        .click();
    h.run();
    assert!(h.state().intents.contains(&Intent::Action(Action::Save)));
}

#[test]
fn scoped_retro_style_does_not_replace_other_frontends_context_styles() {
    let ctx = egui::Context::default();
    paintcraft_ui::fonts::install(&ctx);
    ctx.set_theme(egui::Theme::Dark);
    let before = ctx.global_style().text_styles.clone();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let outside = ui.style().text_styles.clone();
        with_style(ui, |ui| {
            for font in ui.style().text_styles.values() {
                assert_eq!(font.family, paintcraft_ui::fonts::retro_family());
            }
            assert!(!ui.visuals().dark_mode);
        });
        assert_eq!(ui.style().text_styles, outside);
    });
    output.textures_delta.clear();
    assert_eq!(ctx.global_style().text_styles, before);
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(Skin::parse("win95"), Some(Skin::Win95));
    assert_eq!(Skin::parse("future-placeholder"), None);
}

struct CanvasState {
    owner: Id,
    scope: Id,
    size: Vec2,
    rect: Option<Rect>,
    paints: usize,
}
fn canvas_harness() -> Harness<'static, CanvasState> {
    Harness::new_ui_state(
        |ui, state: &mut CanvasState| {
            canvas(
                ui,
                state.owner,
                state.scope,
                state.size,
                |_, response, painter| {
                    state.paints += 1;
                    state.rect = Some(response.rect);
                    painter.rect_filled(response.rect, 0, egui::Color32::WHITE);
                },
            );
        },
        CanvasState {
            owner: Id::unique("first"),
            scope: Id::unique("view1"),
            size: Vec2::new(1200.0, 1000.0),
            rect: None,
            paints: 0,
        },
    )
}
#[test]
fn real_canvas_paints_and_pan_is_owned_by_the_supplied_document_id() {
    let mut h = canvas_harness();
    h.set_size(Vec2::new(320.0, 240.0));
    h.run();
    let first = h.state().rect.unwrap();
    let id = h.state().owner;
    pan_by(&h.ctx, id, Vec2::new(-60.0, -40.0));
    h.run();
    let panned = h.state().rect.unwrap();
    assert!((panned.min - first.min - Vec2::new(-60.0, -40.0)).length() < 1.5);
    h.state_mut().owner = Id::unique("second");
    h.run();
    assert!((h.state().rect.unwrap().min - first.min).length() < 1.5);
    h.state_mut().owner = id;
    h.run();
    assert!((h.state().rect.unwrap().min - panned.min).length() < 1.5);
    assert!(h.state().paints > 0);
}
#[test]
fn real_canvas_zoom_anchor_preserves_the_supplied_image_point_on_screen() {
    let mut h = canvas_harness();
    h.set_size(Vec2::new(320.0, 240.0));
    h.run();
    let point = egui::pos2(60.0, 40.0);
    let screen = h.state().rect.unwrap().min + point.to_vec2();
    let id = h.state().owner;
    let scope = Id::unique("zoom2");
    h.state_mut().scope = scope;
    h.state_mut().size *= 2.0;
    anchor_at(&h.ctx, id, scope, point, screen, 2.0);
    h.run();
    let actual = h.state().rect.unwrap().min + point.to_vec2() * 2.0;
    assert!((actual - screen).length() < 1.5, "{actual:?} != {screen:?}");
    assert!(
        h.ctx
            .data(|data| data.get_temp::<crate::anchor::Anchor>(id.with("retro-anchor-request")))
            .is_none()
    );
}

#[test]
fn explicit_browser_file_capability_omits_recent_and_both_menus_omit_invented_close() {
    for browser in [false, true] {
        let mut h = Harness::new_ui_state(
            move |ui, state: &mut State| {
                initialize(ui, state);
                let mut ctx = context(false);
                ctx.can_recent = true;
                state.intents.extend(if browser {
                    menu_bar_for_browser(ui, &mut state.workspace, ctx)
                } else {
                    menu_bar(ui, &mut state.workspace, ctx)
                });
            },
            State::default(),
        );
        h.run();
        h.get_by_label("File").click();
        h.run();
        assert!(h.query_by_label("Close").is_none());
        assert!(h.query_by_label("Exit").is_none());
        assert_eq!(h.query_by_label("Recent files").is_some(), !browser);
        let shortcut = Action::Save.descriptor().shortcut.unwrap();
        h.get_by_label(&format!("Save {}", h.ctx.format_shortcut(&shortcut)))
            .click();
        h.run();
        assert!(h.state().intents.contains(&Intent::Action(Action::Save)));
    }
}
#[test]
fn real_palette_hover_has_one_named_tooltip_and_keeps_accessible_color_actions() {
    for (accessible, tooltip, rgba) in [
        ("Palette color #800000", "Maroon #800000", [128, 0, 0, 255]),
        ("Palette color #00FFFF", "Cyan #00FFFF", [0, 255, 255, 255]),
    ] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(900.0, 250.0))
            .build_ui_state(
                |ui, state: &mut State| {
                    initialize(ui, state);
                    ui.style_mut().interaction.tooltip_delay = 0.0;
                    ui.style_mut().interaction.tooltip_grace_time = 0.0;
                    ui.style_mut().interaction.show_tooltips_only_when_still = false;
                    state.intents.extend(palette(
                        ui,
                        &mut state.controls,
                        &mut state.workspace,
                        context(false),
                    ));
                },
                State::default(),
            );
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(egui::Role::ColorWell, accessible)
                .count(),
            1
        );
        h.get_by_role_and_label(egui::Role::ColorWell, accessible)
            .hover();
        h.run();
        assert_eq!(h.query_all_by_role(egui::Role::Tooltip).count(), 1);
        let popup = h.get_by_role(egui::Role::Tooltip);
        assert_eq!(popup.query_all_by_label(tooltip).count(), 1);
        // The generic accessibility name belongs to the swatch, not a second
        // tooltip label (whether appended to this popup or shown separately).
        assert!(popup.query_by_label(accessible).is_none());
        h.get_by_role_and_label(egui::Role::ColorWell, accessible)
            .click();
        h.run();
        assert_eq!(h.state().controls.primary, Rgba(rgba));
        h.get_by_role_and_label(egui::Role::ColorWell, accessible)
            .click_secondary();
        h.run();
        assert_eq!(h.state().controls.secondary, Rgba(rgba));
        assert!(h.state().intents.is_empty());
    }
}

#[test]
fn compact_tool_and_palette_geometry_keep_selected_accessibility_and_real_colors() {
    let mut tools = Harness::builder()
        .with_size(Vec2::new(58.0, 420.0))
        .build_ui_state(
            |ui, state: &mut State| {
                initialize(ui, state);
                toolbox(
                    ui,
                    &mut state.controls,
                    &mut state.workspace,
                    context(false),
                );
            },
            State::default(),
        );
    tools.run();
    let first = tools
        .get_by_role_and_label(egui::Role::Button, "Free-form")
        .rect();
    let second = tools
        .get_by_role_and_label(egui::Role::Button, "Select")
        .rect();
    assert!((first.width() - 25.0).abs() < 1.0 && (second.min.x - first.min.x - 25.0).abs() < 1.0);
    let pencil = tools.get_by_role_and_label(egui::Role::Button, "Pencil");
    assert_eq!(
        pencil.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    let rect = pencil.rect();
    assert!(tools.output().shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rect && r.fill==egui::Color32::from_gray(192))));
    let mut colors = Harness::builder()
        .with_size(Vec2::new(320.0, 100.0))
        .build_ui_state(
            |ui, state: &mut State| {
                initialize(ui, state);
                palette(
                    ui,
                    &mut state.controls,
                    &mut state.workspace,
                    context(false),
                );
            },
            State::default(),
        );
    colors.run();
    let black = colors
        .get_by_role_and_label(egui::Role::ColorWell, "Palette color #000000")
        .rect();
    let gray = colors
        .get_by_role_and_label(egui::Role::ColorWell, "Palette color #808080")
        .rect();
    let brown = colors
        .get_by_role_and_label(egui::Role::ColorWell, "Palette color #804000")
        .rect();
    let white = colors
        .get_by_role_and_label(egui::Role::ColorWell, "Palette color #FFFFFF")
        .rect();
    assert!((black.width() - 16.0).abs() < 1.0);
    assert!((gray.min.x - black.min.x - 17.0).abs() < 1.0);
    assert!((brown.max.x - black.min.x - 237.0).abs() < 1.0);
    assert!(
        (white.min.x - black.min.x).abs() < 1.0 && (white.min.y - black.min.y - 17.0).abs() < 1.0
    );
    assert!(colors.ctx.content_rect().contains_rect(white));
    let current = colors.get_by_role_and_label(egui::Role::ColorWell, "Color 1");
    assert_eq!(
        current.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    let rect = current.rect();
    assert!(colors.output().shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rect && r.fill==egui::Color32::from_gray(192))));
    assert!(colors.output().shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rect.shrink(4.0) && r.fill==egui::Color32::BLACK)));
}

#[test]
fn short_tool_strip_scrolls_to_the_real_lower_tool_choices() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(58.0, 120.0))
        .with_max_steps(12)
        .build_ui_state(
            |ui, state: &mut State| {
                initialize(ui, state);
                state.intents.extend(toolbox(
                    ui,
                    &mut state.controls,
                    &mut state.workspace,
                    context(false),
                ));
            },
            State::default(),
        );
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Free-form")
        .hover();
    h.run();
    h.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -240.0),
        modifiers: egui::Modifiers::NONE,
        source: egui::MouseWheelSource::Unknown,
        phase: egui::TouchPhase::Move,
    });
    h.run();
    let last = ToolChoice::RoundedRectangle;
    let button = h.get_by_role_and_label(egui::Role::Button, last.label());
    assert!(h.ctx.content_rect().contains_rect(button.rect()));
    button.click();
    h.run();
    assert!(h.state().intents.contains(&Intent::ChooseTool(last)));
}

#[test]
fn brush_square_option_keeps_its_full_label_on_one_compact_row() {
    let mut state = State::default();
    ToolChoice::Brush.apply_to(&mut state.controls);
    let mut h = Harness::builder()
        .with_size(Vec2::new(58.0, 420.0))
        .build_ui_state(
            |ui, state: &mut State| {
                initialize(ui, state);
                toolbox(
                    ui,
                    &mut state.controls,
                    &mut state.workspace,
                    context(false),
                );
            },
            state,
        );
    h.run();
    let square = h.get_by_role_and_label(egui::Role::RadioButton, "Square");
    let round = h.get_by_role_and_label(egui::Role::RadioButton, "Round");
    assert!(square.rect().height() <= 20.0, "{:?}", square.rect());
    assert!(square.rect().width() <= 50.0, "{:?}", square.rect());
    assert!((square.rect().height() - round.rect().height()).abs() < 1.0);
    square.click();
    h.run();
    assert_eq!(h.state().controls.tool, Tool::Brush);
    assert!(!h.state().controls.options.round);
    assert!(h.state().intents.is_empty());
}
