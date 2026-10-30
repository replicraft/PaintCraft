//! Fresh multi-size assets. Foreign preserving saves are never implied.
use super::*;
use paintcraft_formats::{
    icons,
    registry::{self, Family, Operation, Platform, Request},
};

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    metadata: super::export_metadata::Options,
    /// Repeat SIZE:png|bitmap or WIDTHxHEIGHT:png|bitmap. CUR adds :HOTSPOT_X:HOTSPOT_Y.
    #[arg(long, required = true, value_parser = parse_entry)]
    entry: Vec<icons::EntrySpec>,
    /// Resize each entry from the original canvas, fitting on transparent padding.
    #[arg(long, value_enum, default_value = "lanczos3")]
    resampling: Resampling,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: super::format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Resampling {
    Nearest,
    Lanczos3,
}
impl From<Resampling> for icons::Filter {
    fn from(value: Resampling) -> Self {
        match value {
            Resampling::Nearest => Self::Nearest,
            Resampling::Lanczos3 => Self::Lanczos3,
        }
    }
}
fn parse_entry(value: &str) -> std::result::Result<icons::EntrySpec, String> {
    let fields: Vec<_> = value.split(':').collect();
    if ![2, 4].contains(&fields.len()) {
        return Err(
            "entry must be SIZE:png|bitmap or WIDTHxHEIGHT:png|bitmap with optional :X:Y hotspot"
                .into(),
        );
    }
    let (width, height) = fields[0].split_once('x').unwrap_or((fields[0], fields[0]));
    let side = |value: &str| {
        value
            .parse::<u32>()
            .ok()
            .filter(|size| (1..=256).contains(size))
            .ok_or_else(|| "entry dimensions must be between 1 and 256".to_owned())
    };
    let width = side(width)?;
    let height = side(height)?;
    let encoding = match fields[1] {
        "png" => icons::Encoding::Png,
        "bitmap" => icons::Encoding::Bitmap,
        _ => return Err("entry encoding must be png or bitmap".into()),
    };
    let hotspot = if fields.len() == 4 {
        let x = fields[2].parse::<u16>().map_err(|_| "invalid hotspot x")?;
        let y = fields[3].parse::<u16>().map_err(|_| "invalid hotspot y")?;
        if u32::from(x) >= width || u32::from(y) >= height {
            return Err("hotspot must be inside its entry".into());
        }
        Some((x, y))
    } else {
        None
    };
    Ok(icons::EntrySpec {
        width,
        height,
        encoding,
        hotspot,
    })
}

pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = a.output.output.as_path();
            if same_file(&a.input, dest) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "icon output must not replace its source",
                ));
            }
            let family = dest
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Family::from_extension)
                .filter(|family| matches!(family, Family::Ico | Family::Cur))
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::InputSchema,
                        "icon output requires an .ico or .cur extension",
                    )
                })?;
            let mut request = Request::new(
                a.profile.frontend.into(),
                Operation::SaveAsCopy,
                family,
                Platform::Native,
            );
            request.extension = dest.extension().and_then(|ext| ext.to_str());
            request.variant = Some("icon-fresh-entry-set");
            let decision = registry::admission(request);
            if !decision.available() {
                decision.require_allowed(request)?;
            }
            let allowed = a.losses.allowed_for(icons::KNOWN_EXPORT_LOSSES)?;
            save::preflight(dest, a.output.overwrite)?;
            let (bytes, sha) = read_bounded(&a.input, INPUT_MAX, "input")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            if a.profile.admit_open(&a.input, &bytes)? != Family::Pcnd {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "icon export expects a PCND project",
                ));
            }
            let doc = super::source_metadata::read(&bytes)?;
            let kind = if family == Family::Ico {
                icons::Kind::Icon
            } else {
                icons::Kind::Cursor
            };
            let policy = a.metadata.capture_policies(
                &doc,
                super::export_metadata::CopyFormat::Icon(kind),
                a.losses.filter_bake_intent(),
                &|| Ok(()),
            )?;
            let prepared = icons::prepare_document_with_policies(
                &doc,
                kind,
                &a.entry,
                a.resampling.into(),
                icons::WriteLimits::default(),
                &|| Ok(()),
                policy,
            )?;
            let encoded = prepared.encode(&allowed, &|| Ok(()))?;
            let ctx = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: Some(sha),
                losses_acknowledged: prepared.losses().to_vec(),
                destination: dest,
            };
            let (receipt, ok, summary) = write_output(dest, a.output.overwrite, Ok(encoded), &ctx);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
