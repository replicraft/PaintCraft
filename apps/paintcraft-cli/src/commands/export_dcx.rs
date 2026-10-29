//! Explicit one-composite-page DCX copies, never layers mapped to pages.
use super::*;
use paintcraft_formats::{
    dcx, export, pcx,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};

#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    Rgb24,
    IndexedExact,
    Quantized256,
}
impl From<Mode> for pcx::Mode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Rgb24 => Self::Rgb24,
            Mode::IndexedExact => Self::IndexedExact,
            Mode::Quantized256 => Self::Quantized256,
        }
    }
}
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    metadata: super::export_metadata::Options,
    /// Explicit one-page PCX encoding; quantization and alpha conversion need actual loss consent.
    #[arg(long, value_enum)]
    pcx_mode: Mode,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
pub(super) fn admit(frontend: Frontend, path: &Path) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::SaveAsCopy,
        Family::Dcx,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|value| value.to_str());
    if request.extension.is_none() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "DCX output requires .dcx extension",
        ));
    }
    request.variant = Some(dcx::EXPORT_VARIANT);
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
/// Source guards run only before preparation/staging and at publication. The
/// encoder's cheap cancellation callback performs no filesystem metadata calls.
#[allow(clippy::too_many_arguments)] // Separate cheap cancellation from filesystem identity checks.
pub(super) fn write_copy(
    doc: &Document,
    mode: Mode,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    protect: &dyn Fn() -> Result<()>,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Value, bool, String)> {
    write_copy_with_metadata_policy(
        doc,
        mode,
        dest,
        overwrite,
        allowed,
        source_sha,
        protect,
        check,
        export::metadata_policy::Policy::default(),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn write_copy_with_metadata_policy(
    doc: &Document,
    mode: Mode,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    protect: &dyn Fn() -> Result<()>,
    check: &dyn Fn() -> Result<()>,
    policy: export::metadata_policy::Policy,
) -> Result<(Value, bool, String)> {
    check()?;
    protect()?;
    let limits = dcx::DocumentLimits::default();
    let prepared = dcx::prepare_document_with_metadata_policy(
        doc,
        mode.into(),
        allowed,
        limits,
        check,
        |doc| paintcraft_render::render_cancellable(doc, check),
        policy,
    )?;
    let context = Context {
        operation: "export",
        source_state: doc.state_id,
        source_sha256: source_sha,
        losses_acknowledged: prepared.losses().to_vec(),
        destination: dest,
    };
    let outcome = save::save_with_preparation(
        dest,
        overwrite,
        limits.encoded_bytes as u64,
        |sink| {
            check()?;
            protect()?;
            for chunk in prepared.bytes().chunks(65536) {
                check()?;
                sink.write_all(chunk)
                    .map_err(|error| storage("staged DCX copy", &error))?;
            }
            check()
        },
        |_| {
            check()?;
            protect()?;
            check()
        },
    );
    Ok(finish_output(dest, &context, outcome))
}
pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = &a.output.output;
            admit(a.profile.frontend.into(), dest)?;
            let allowed = a.losses.allowed_for(&export::KNOWN_LOSSES)?;
            let (bytes, sha, identity) = read_captured(&a.input, INPUT_MAX, "DCX source project")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            let protect = || {
                if identity.matches_path(dest)? {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "DCX copy must not replace its captured source project",
                    ));
                }
                Ok(())
            };
            protect()?;
            save::preflight(dest, a.output.overwrite)?;
            if a.profile.admit_open(&a.input, &bytes)? != Family::Pcnd {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "DCX export expects a PCND project",
                ));
            }
            let doc = super::source_metadata::read(&bytes)?;
            let policy = a
                .metadata
                .capture(&doc, super::export_metadata::CopyFormat::Dcx)?;
            let (receipt, ok, summary) =
                if a.metadata.intent == export::metadata_policy::Intent::Refuse {
                    write_copy(
                        &doc,
                        a.pcx_mode,
                        dest,
                        a.output.overwrite,
                        &allowed,
                        Some(sha),
                        &protect,
                        &|| Ok(()),
                    )?
                } else {
                    write_copy_with_metadata_policy(
                        &doc,
                        a.pcx_mode,
                        dest,
                        a.output.overwrite,
                        &allowed,
                        Some(sha),
                        &protect,
                        &|| Ok(()),
                        policy,
                    )?
                };

            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn late_source_alias_at_publication_is_refused_after_staging() {
        let folder = std::env::temp_dir().join(format!(
            "paintcraft-dcx-late-alias-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&folder).unwrap();
        let source = folder.join("source.pcnd");
        let dest = folder.join("late.dcx");
        let doc = paintcraft_document::new_document(
            3,
            2,
            Some(Rgba([17, 51, 93, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let original = pcnd::write(&doc).unwrap();
        std::fs::write(&source, &original).unwrap();
        let (_, _, identity) = read_captured(&source, INPUT_MAX, "late source").unwrap();
        let calls = Cell::new(0);
        let protect = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                std::fs::hard_link(&source, &dest).unwrap();
            }
            if identity.matches_path(&dest)? {
                return Err(Error::new(ErrorCode::InputSchema, "source alias"));
            }
            Ok(())
        };
        let (_, ok, _) = write_copy(&doc, Mode::Rgb24, &dest, true, &[], None, &protect, &|| {
            Ok(())
        })
        .unwrap();
        assert!(!ok);
        assert_eq!(calls.get(), 3);
        assert_eq!(std::fs::read(&dest).unwrap(), original);
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 2);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn cancellation_during_preparation_and_at_publication_preserves_session_and_destination() {
        let folder =
            std::env::temp_dir().join(format!("paintcraft-dcx-cancel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let dest = folder.join("existing.dcx");
        let original = b"unchanged existing destination";
        std::fs::write(&dest, original).unwrap();
        let doc = paintcraft_document::new_document(
            65,
            3,
            Some(Rgba([17, 51, 93, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let session = paintcraft_editor::session::Session::new(doc);
        let before = session.document().clone();
        let dirty = session.is_dirty();
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            if calls.get() == 5 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancelled preparation",
                ))
            } else {
                Ok(())
            }
        };
        let error = write_copy(
            session.document(),
            Mode::IndexedExact,
            &dest,
            true,
            &[],
            None,
            &|| Ok(()),
            &cancelled,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(std::fs::read(&dest).unwrap(), original);
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);

        let protection_calls = Cell::new(0);
        let stopped = Cell::new(false);
        let protect = || {
            protection_calls.set(protection_calls.get() + 1);
            if protection_calls.get() == 3 {
                stopped.set(true);
            }
            Ok(())
        };
        let check = || {
            if stopped.get() {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancelled at publication",
                ))
            } else {
                Ok(())
            }
        };
        let (_, ok, _) = write_copy(
            session.document(),
            Mode::IndexedExact,
            &dest,
            true,
            &[],
            None,
            &protect,
            &check,
        )
        .unwrap();
        assert!(!ok);
        assert_eq!(protection_calls.get(), 3);
        assert_eq!(std::fs::read(&dest).unwrap(), original);
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.document().state_id, before.state_id);
        assert_eq!(session.is_dirty(), dirty);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
