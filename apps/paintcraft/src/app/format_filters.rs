//! Discovery is derived from the same qualified policy as actual decoding/writing.
use super::*;
use paintcraft_formats::registry::{self, Family, Operation, Platform};
use paintcraft_platform::dialogs::Filter;
use paintcraft_ui::t;

impl App {
    pub(super) fn format_filters(&self, kind: Kind) -> Vec<Filter> {
        let frontend = self.format_frontend();
        if matches!(kind, Kind::Open | Kind::Import) {
            let mut filters = vec![Filter {
                name: if kind == Kind::Import {
                    "Images to import"
                } else {
                    "Images and PaintCraft documents"
                }
                .into(),
                extensions: registry::enabled_extensions(
                    frontend,
                    if kind == Kind::Import {
                        Operation::ImportConvert
                    } else {
                        Operation::Open
                    },
                    Platform::Native,
                )
                .into_iter()
                .filter(|extension| {
                    (kind != Kind::Import
                        || frontend == registry::Frontend::Studio
                        || registry::Family::from_extension(extension) != Some(Family::Msp))
                        && (kind != Kind::Import
                            || registry::Family::from_extension(extension).is_some_and(|family| {
                                !family.is_project()
                                    || paintcraft_formats::project_import::supports(family)
                            }))
                })
                .map(str::to_owned)
                .collect(),
            }];
            if kind == Kind::Import && frontend == registry::Frontend::Studio {
                filters[0].extensions.push("dcx".into());
                filters[0].extensions.push("jxl".into());
                filters[0]
                    .extensions
                    .extend(["ico".into(), "cur".into(), "icns".into()]);
                filters[0].extensions.extend(["svg".into(), "svgz".into()]);
            }
            return filters;
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
                    Family::Gif => "GIF image",
                    Family::Tiff => "TIFF image",
                    Family::Tga => "TGA image",
                    Family::Webp => "WebP image",
                    Family::Jxl => "JPEG XL image",
                    Family::Dds => "DDS image",
                    Family::Avif => "AVIF image",
                    Family::Qoi => "QOI image",
                    Family::Pnm => "PAM image",
                    Family::Farbfeld => "farbfeld image",
                    Family::Pcx => "PCX image",
                    Family::Msp => "MSP picture",
                    _ => return None,
                };
                Some(Filter {
                    name: if family == Family::Msp {
                        t!(&self.context, name)
                    } else {
                        name.into()
                    },
                    extensions: family
                        .extensions()
                        .iter()
                        .filter(|extension| {
                            let mut request = registry::Request::new(
                                frontend,
                                Operation::ExportFlat,
                                family,
                                Platform::Native,
                            );
                            request.extension = Some(extension);
                            registry::admission(request).available()
                        })
                        .map(|ext| (*ext).into())
                        .collect(),
                })
            })
            .collect()
    }
}
