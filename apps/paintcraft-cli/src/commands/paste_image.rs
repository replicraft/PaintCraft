//! A concrete consumer of admitted, frozen external-image placement.
use super::*;
use paintcraft_core::types::{
    clipboard::{Image, MAX_BYTES, MAX_PIXELS},
    resources::SharedBudget,
};
use paintcraft_editor::clipboard::{PasteCanvas, PasteDraft};

#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    image: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[arg(long)]
    expect_image_sha256: Option<String>,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    x: i64,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    y: i64,
    #[arg(long, default_value = "Pasted image")]
    name: String,
    /// Place onto this visible layer UUID instead of creating a new layer.
    #[arg(long, conflicts_with = "name")]
    onto_layer: Option<String>,
    /// Existing-layer alpha policy (default: source-over).
    #[arg(long, value_enum, requires = "onto_layer")]
    blend: Option<Blend>,
    /// Grow right/down to contain the image. Default retains off-canvas pixels.
    #[arg(long)]
    expand_canvas: bool,
    #[command(flatten)]
    profile: super::format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Blend {
    Replace,
    SourceOver,
}
pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = a.output.output.as_path();
            let allowed = a.losses.allowed_for(&image::KNOWN_IMPORT_LOSSES)?;
            save::preflight(dest, a.output.overwrite)?;
            if same_file(&a.image, dest) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "output cannot replace the source image",
                ));
            }
            let in_place = same_file(&a.input, dest);
            let (document_bytes, sha, project_identity) =
                read_regular_or_bounded(&a.input, INPUT_MAX, "input")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            let captured_limit = a
                .image
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                .and_then(|family| match family {
                    paintcraft_formats::registry::Family::Msp => {
                        Some(paintcraft_formats::msp::BYTES_MAX)
                    }
                    paintcraft_formats::registry::Family::Dds => {
                        Some(paintcraft_formats::dds::BYTES_MAX)
                    }
                    _ => None,
                });
            let (image_bytes, image_sha, source_identity) = if let Some(limit) = captured_limit {
                let (bytes, sha, identity) =
                    read_captured(&a.image, limit as u64, "captured image")?;
                (bytes, sha, Some(identity))
            } else {
                read_regular_or_bounded(&a.image, 64 << 20, "image")?
            };
            guard_named(
                a.expect_image_sha256.as_deref(),
                &image_sha,
                "--expect-image-sha256",
            )?;
            let mut session = Session::new(super::source_metadata::read(&document_bytes)?);
            let family = a.profile.admit_import(&a.image, &image_bytes)?;
            if matches!(
                family,
                paintcraft_formats::registry::Family::Dds
                    | paintcraft_formats::registry::Family::Msp
            ) && source_identity.is_none()
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "DDS/MSP paste requires a captured regular source",
                ));
            }
            a.profile.reject_project_import(family)?;
            let decoded = image::decode_with_pixel_limit(&image_bytes, MAX_PIXELS)?;
            if let Some(loss) = decoded.losses.iter().find(|loss| !allowed.contains(loss)) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    format!("import would lose {loss}; acknowledge with --allow-loss {loss}"),
                ));
            }
            let image = Image::from_rgba(
                decoded.width,
                decoded.height,
                decoded.rgba,
                &SharedBudget::new(MAX_BYTES),
                &|| Ok(()),
            )?;
            let mut draft = PasteDraft::new(&session, image)?;
            draft.place(
                (a.x, a.y),
                if a.expand_canvas {
                    PasteCanvas::Grow
                } else {
                    PasteCanvas::Keep
                },
            )?;
            let engine = if let Some(target) = &a.onto_layer {
                let target = paintcraft_core::types::ids::parse_uuid(target).ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "--onto-layer requires a layer UUID")
                })?;
                let mode = match a.blend.unwrap_or(Blend::SourceOver) {
                    Blend::Replace => paintcraft_core::types::policy::BlendMode::Replace,
                    Blend::SourceOver => paintcraft_core::types::policy::BlendMode::SourceOver,
                };
                session.paste_image_onto(&draft, target, mode, None)?
            } else {
                session.paste_image(&draft, &a.name)?
            };
            let ctx = Context {
                operation: if in_place { "save" } else { "save-as" },
                source_state: session.document().state_id,
                source_sha256: Some(sha),
                losses_acknowledged: decoded.losses,
                destination: dest,
            };
            if source_identity
                .as_ref()
                .map(|identity| identity.matches_path(dest))
                .transpose()?
                .unwrap_or(false)
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "output cannot replace the captured source image",
                ));
            }
            if family == paintcraft_formats::registry::Family::Dds {
                use sha2::{Digest, Sha256};
                source_identity
                    .as_ref()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorCode::InputSchema,
                            "DDS paste requires a captured regular source",
                        )
                    })?
                    .verify_bytes(
                        &a.image,
                        image_bytes.len() as u64,
                        &Sha256::digest(&image_bytes).into(),
                        &|| Ok(()),
                    )?;
            }
            let (receipt, ok, summary) =
                write_native_guarded(dest, a.output.overwrite, session.document(), &ctx, || {
                    if let Some(project_identity) = &project_identity {
                        if !in_place && project_identity.matches_path(dest)? {
                            return Err(Error::new(
                                ErrorCode::InputSchema,
                                "copy output cannot replace the captured source project",
                            ));
                        }
                        project_identity.verify_bytes(
                            &a.input,
                            document_bytes.len() as u64,
                            &Sha256::digest(&document_bytes).into(),
                            &|| Ok(()),
                        )?;
                    }
                    if let Some(identity) = &source_identity {
                        if identity.matches_path(dest)? {
                            return Err(Error::new(
                                ErrorCode::InputSchema,
                                "output cannot replace the captured source image",
                            ));
                        }
                        if family == paintcraft_formats::registry::Family::Dds {
                            identity.verify_bytes(
                                &a.image,
                                image_bytes.len() as u64,
                                &Sha256::digest(&image_bytes).into(),
                                &|| Ok(()),
                            )?;
                        }
                    }
                    Ok(())
                });
            Ok(Written {
                engine: Some(engine.to_value()),
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
