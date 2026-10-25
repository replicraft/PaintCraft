//! Discovery is derived from the same qualified policy as actual decoding/writing.
use super::*;
use paintcraft_formats::registry::{self, Family, Operation, Platform};
use paintcraft_platform::dialogs::Filter;

impl App {
    pub(super) fn format_filters(&self, kind: Kind) -> Vec<Filter> {
        let frontend = self.format_frontend();
        if kind == Kind::Open {
            return vec![Filter {
                name: "Images and PaintCraft documents".into(),
                extensions: registry::enabled_extensions(
                    frontend,
                    Operation::Open,
                    Platform::Native,
                )
                .into_iter()
                .map(str::to_owned)
                .collect(),
            }];
        }
        if kind != Kind::Export {
            return vec![Filter {
                name: "PaintCraft document".into(),
                extensions: vec!["pcnd".into()],
            }];
        }
        registry::enabled(frontend, Operation::ExportFlat, Platform::Native)
            .into_iter()
            .filter_map(|family| {
                let name = match family {
                    Family::Png => "PNG image",
                    Family::Jpeg => "JPEG image",
                    Family::Bmp => "BMP image",
                    _ => return None,
                };
                Some(Filter {
                    name: name.into(),
                    extensions: family
                        .extensions()
                        .iter()
                        .filter(|extension| family.qualified_extension(extension))
                        .map(|ext| (*ext).into())
                        .collect(),
                })
            })
            .collect()
    }
}
