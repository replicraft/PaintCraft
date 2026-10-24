//! Interface-separated, embedded Fluent resources shared by desktop and web.
use super::Language;

pub(crate) fn sources(language: Language) -> &'static [&'static str] {
    match language {
        Language::English => &[
            include_str!("../../locales/en/app.ftl"),
            include_str!("../../locales/en/common.ftl"),
            include_str!("../../locales/en/classic.ftl"),
            include_str!("../../locales/en/studio.ftl"),
            include_str!("../../locales/en/settings.ftl"),
        ],
        Language::Japanese => &[
            include_str!("../../locales/ja/app.ftl"),
            include_str!("../../locales/ja/common.ftl"),
            include_str!("../../locales/ja/classic.ftl"),
            include_str!("../../locales/ja/studio.ftl"),
            include_str!("../../locales/ja/settings.ftl"),
        ],
        Language::German => &[
            include_str!("../../locales/de/app.ftl"),
            include_str!("../../locales/de/common.ftl"),
            include_str!("../../locales/de/classic.ftl"),
            include_str!("../../locales/de/studio.ftl"),
            include_str!("../../locales/de/settings.ftl"),
        ],
        Language::Spanish => &[
            include_str!("../../locales/es/app.ftl"),
            include_str!("../../locales/es/common.ftl"),
            include_str!("../../locales/es/classic.ftl"),
            include_str!("../../locales/es/studio.ftl"),
            include_str!("../../locales/es/settings.ftl"),
        ],
        Language::PortugueseBrazil => &[
            include_str!("../../locales/pt-BR/app.ftl"),
            include_str!("../../locales/pt-BR/common.ftl"),
            include_str!("../../locales/pt-BR/classic.ftl"),
            include_str!("../../locales/pt-BR/studio.ftl"),
            include_str!("../../locales/pt-BR/settings.ftl"),
        ],
        Language::Hindi => &[
            include_str!("../../locales/hi/app.ftl"),
            include_str!("../../locales/hi/common.ftl"),
            include_str!("../../locales/hi/classic.ftl"),
            include_str!("../../locales/hi/studio.ftl"),
            include_str!("../../locales/hi/settings.ftl"),
        ],
        Language::ChineseSimplified => &[
            include_str!("../../locales/zh-CN/app.ftl"),
            include_str!("../../locales/zh-CN/common.ftl"),
            include_str!("../../locales/zh-CN/classic.ftl"),
            include_str!("../../locales/zh-CN/studio.ftl"),
            include_str!("../../locales/zh-CN/settings.ftl"),
        ],
    }
}
