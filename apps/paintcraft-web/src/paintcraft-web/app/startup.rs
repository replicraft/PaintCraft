//! Actual preference-selected startup; persisted choices never restore document pixels.
use super::*;
impl WebApp {
    pub fn new(ctx: &egui::Context) -> Self {
        let (preferences, warning) = crate::preferences::Preferences::load();
        Self::from_preferences(ctx, preferences, warning)
    }
    pub(super) fn from_preferences(
        ctx: &egui::Context,
        preferences: crate::preferences::Preferences,
        warning: Option<String>,
    ) -> Self {
        if let Some(theme) = preferences.theme {
            ctx.set_theme(theme);
        }
        let (width, height) = Self::initial_size(preferences.frontend, &preferences);
        set_language(ctx, preferences.language);
        paintcraft_ui::fonts::install_for_locale(ctx, preferences.language.id());
        paintcraft_ui::theme::apply(ctx, ctx.theme());
        Self {
            frontend: preferences.frontend,
            studio: Default::default(),
            retro: paintcraft_retro::Workspace::new(preferences.retro_skin),
            filter_editing: None,
            filter_display: Default::default(),
            parked: Default::default(),
            fullscreen: Default::default(),
            printer: Default::default(),
            controls: Controls {
                tool: Tool::Brush,
                options: ToolOptions {
                    pen: preferences.pen,
                    ..Default::default()
                },
                primary: paintcraft_core::types::Rgba([0, 0, 0, 255]),
                secondary: WHITE,
            },
            editor: Editor::new(width, height).expect("validated startup canvas is admitted"),
            canvas: Canvas::default(),
            pen: crate::pen_input::State::new(ctx),
            workspace: Workspace {
                custom_colors: preferences.custom_colors,
                ..Workspace::default()
            },
            variant: preferences.variant,
            name: "Untitled".into(),
            zoom: 1.0,
            settings: false,
            welcome: welcome::State::load(),
            applied_style: None,
            settings_draft: None,
            text: crate::text_ui::TextUi::with_style(paintcraft_classic::TextOptions {
                family: preferences.default_font.clone(),
                ..Default::default()
            }),
            preferences,
            export_confirm: None,
            export_frozen: None,
            export_omit_source_metadata: false,
            export_bake_filters: false,
            dds: Default::default(),
            dds_settings: Default::default(),
            msp: Default::default(),
            msp_version: Default::default(),
            icon_export: None,
            icns_export: None,
            palettes: Default::default(),
            shapes: Default::default(),
            brushes: Default::default(),
            export_quality: 90,
            export_jpeg_matte: [255; 3],
            export_pcx_encoding: Default::default(),
            pcx_preview: Default::default(),
            confirm_replace: None,
            files: crate::files::Files::default(),
            pending_import: None,
            pending_pages: None,
            pending_icns: None,
            pending_svg: None,
            import_owner: None,
            jxl: Default::default(),
            tiff: Default::default(),
            form: None,
            clipboard: Default::default(),
            paste: Default::default(),
            message: warning.unwrap_or_else(|| {
                "Ready - Save downloads a project copy; work stays in this tab".into()
            }),
        }
    }
}
