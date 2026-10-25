//! CLI/MCP SVG rasterization parameters and explicitly authorized font files.
use super::*;
use paintcraft_formats::svg;

#[derive(
    Clone, Copy, Debug, Default, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    #[default]
    Contain,
    Stretch,
}
impl From<Fit> for svg::Fit {
    fn from(value: Fit) -> Self {
        match value {
            Fit::Contain => Self::Contain,
            Fit::Stretch => Self::Stretch,
        }
    }
}
fn default_dpi() -> f32 {
    96.0
}
#[derive(Clone, Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_dpi")]
    pub dpi: f32,
    #[serde(default)]
    pub fit: Fit,
    /// Additional local font files. Bundled Selawik regular/bold are always available.
    #[serde(default)]
    pub font_files: Vec<PathBuf>,
}
impl Selection {
    pub fn options(&self) -> Result<svg::Options> {
        let options = svg::Options {
            width: self.width,
            height: self.height,
            dpi: self.dpi,
            fit: self.fit.into(),
        };
        options.admit(svg::PIXELS_MAX)?;
        if self.font_files.len() > 30 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "SVG permits at most 30 additional font files",
            ));
        }
        Ok(options)
    }
    pub fn protect_output(&self, path: &Path) -> Result<()> {
        if self.font_files.iter().any(|font| same_file(font, path)) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "SVG conversion must not replace a supplied font file",
            ));
        }
        Ok(())
    }
    pub fn convert(&self, bytes: &[u8], source_sha256: &str) -> Result<(Document, Vec<String>)> {
        let options = self.options()?;
        let mut additional = Vec::new();
        // Leave room for both bundled faces inside the adapter's aggregate cap.
        let mut remaining = (svg::FONT_BYTES_MAX - svg::Fonts::bundled_bytes()) as u64;
        for path in &self.font_files {
            let (data, _) = read_bounded(path, remaining, "SVG font")?;
            remaining -= data.len() as u64;
            additional.push(data);
        }
        let refs = additional.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let fonts = svg::Fonts::with_bundled(&refs, &|| Ok(()))?;
        let raster = svg::prepare(bytes, options, &fonts, svg::PIXELS_MAX, &|| Ok(()))?
            .render(&|| Ok(()))?;
        Ok((raster.document(source_sha256, &|| Ok(()))?, raster.losses))
    }
}
#[derive(Debug, Default, clap::Args)]
#[group(id = "svg-options")]
pub struct Arguments {
    /// Explicit SVG/SVGZ raster width; requires svg-height. Other formats refuse SVG options.
    #[arg(long,requires="svg_height",conflicts_with_all=["page","entry"])]
    svg_width: Option<u32>,
    #[arg(long,requires="svg_width",conflicts_with_all=["page","entry"])]
    svg_height: Option<u32>,
    /// Physical-unit resolution, default 96 when rasterizing SVG.
    #[arg(long, requires = "svg_width")]
    svg_dpi: Option<f32>,
    /// Contain adds transparent padding; stretch scales both axes independently.
    #[arg(long, value_enum, requires = "svg_width")]
    svg_fit: Option<Fit>,
    /// Add an explicitly authorized font file; repeat for more faces.
    #[arg(long = "svg-font", requires = "svg_width")]
    svg_fonts: Vec<PathBuf>,
}
impl Arguments {
    pub fn selection(self) -> Result<Option<Selection>> {
        match (self.svg_width, self.svg_height) {
            (None, None)
                if self.svg_dpi.is_none()
                    && self.svg_fit.is_none()
                    && self.svg_fonts.is_empty() =>
            {
                Ok(None)
            }
            (Some(width), Some(height)) => {
                let selection = Selection {
                    width,
                    height,
                    dpi: self.svg_dpi.unwrap_or(96.0),
                    fit: self.svg_fit.unwrap_or_default(),
                    font_files: self.svg_fonts,
                };
                selection.options()?;
                Ok(Some(selection))
            }
            _ => Err(Error::new(
                ErrorCode::InputSchema,
                "SVG import requires explicit svg-width and svg-height",
            )),
        }
    }
}
