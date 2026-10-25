//! Shared application widgets, view mapping and exactly-once action delivery.
pub mod actions;
pub mod chrome;
pub mod confirmation;
pub mod fonts;
pub mod icons;
pub mod outbox;
pub mod page_import;
pub mod pcx_export;
pub mod view;
pub mod widgets;

pub mod theme;

pub mod i18n;

/// Translate UI text with the language of an egui context, using Fluent.
///
/// `t!(ctx, "Settings")` translates a label. Named arguments preserve user data:
/// `t!(ctx, "Opened { $detail }", detail = filename)`.
/// Evaluate translations before context-locked accessibility callbacks.
#[macro_export]
macro_rules! t {
    ($ctx:expr, $source:expr $(,)?) => {
        $crate::i18n::tr($ctx, $source)
    };
    ($ctx:expr, $source:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tr_format($ctx, $source, &[$((stringify!($name), ($value).to_string())),+])
    };
    ($ctx:expr, $source:expr, $values:expr $(,)?) => {
        $crate::i18n::tr_format($ctx, $source, $values)
    };
}
