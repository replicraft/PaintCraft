//! Private float controls retain the captured source and never publish a preview.
use super::*;
use paintcraft_formats::precision::{
    AlphaAssignment, AlphaOutput, Curve, LinearSpace, Negative, Settings,
};

pub(super) struct Options {
    input: PathBuf,
    identity: SourceIdentity,
    length: u64,
    codec: wire::Codec,
    settings: Settings,
    pub(super) error: Option<Error>,
}
impl Options {
    fn show(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let before = self.settings;
        let ctx = ui.ctx().clone();
        ui.label(t!(
            &ctx,
            "Convert to an 8-bit sRGB picture. Preview before importing."
        ));
        choice(
            ui,
            "Source color space",
            &mut self.settings.source_assignment,
            &[
                (None, "Use source metadata"),
                (Some(LinearSpace::Rec709D65), "Linear Rec.709 / D65"),
                (
                    Some(LinearSpace::RadianceEqualEnergy),
                    "Linear Radiance / equal-energy white",
                ),
            ],
        );
        choice(
            ui,
            "Source transparency",
            &mut self.settings.alpha_assignment,
            &[
                (None, "Use source metadata"),
                (Some(AlphaAssignment::Premultiplied), "Premultiplied alpha"),
                (Some(AlphaAssignment::Straight), "Straight alpha"),
            ],
        );
        choice(
            ui,
            "Tone mapping",
            &mut self.settings.curve,
            &[
                (Curve::Clip, "Clip to display range"),
                (Curve::ReinhardPerChannel, "Reinhard per channel"),
            ],
        );
        ui.horizontal(|ui| {
            let label = t!(&ctx, "Exposure (EV)");
            let caption = ui.label(&label);
            let mut ev = f64::from(self.settings.exposure_milli_ev) / 1000.0;
            let response = ui.add(
                egui::DragValue::new(&mut ev)
                    .range(-16.0..=16.0)
                    .speed(0.1)
                    .fixed_decimals(3),
            );
            let response = response.labelled_by(caption.id);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), &label)
            });
            if response.changed() && ev.is_finite() {
                self.settings.exposure_milli_ev = (ev * 1000.0).round() as i32;
            }
        });
        choice(
            ui,
            "Negative samples",
            &mut self.settings.negative,
            &[
                (Negative::Reject, "Refuse negative samples"),
                (Negative::ClipToBlack, "Clip negative samples to black"),
            ],
        );
        let mut flatten = matches!(self.settings.alpha_output, AlphaOutput::Flatten { .. });
        if ui
            .checkbox(&mut flatten, t!(&ctx, "Flatten transparency onto a color"))
            .changed()
        {
            self.settings.alpha_output = if flatten {
                AlphaOutput::Flatten {
                    srgb_background: [255; 3],
                }
            } else {
                AlphaOutput::Preserve
            };
        }
        if let AlphaOutput::Flatten { srgb_background } = &mut self.settings.alpha_output {
            ui.horizontal(|ui| {
                ui.label(t!(&ctx, "Background color"));
                ui.color_edit_button_srgb(srgb_background);
            });
        }
        if let Some(error) = &self.error {
            let text = t!(&ctx, refusal_text(error));
            ui.colored_label(ui.visuals().error_fg_color, text);
        }
        let changed = before != self.settings;
        (changed, ui.button(t!(&ctx, "Prepare preview")).clicked())
    }
}
fn refusal_text(error: &Error) -> &'static str {
    let detail = error.to_string();
    if detail.contains("Choose the image's source color space") {
        "Choose the image's source color space before conversion"
    } else if detail.contains("Negative source values")
        || detail.contains("Out-of-gamut negative color")
    {
        "This image needs an explicit negative-value policy. Choose clipping to continue."
    } else if detail.contains("Zero-alpha additive color") {
        "This image contains additive transparent color. Choose a background to flatten it."
    } else if detail.contains("Nonfinite") || detail.contains("Float alpha must") {
        "This image contains invalid color or transparency values and cannot be converted."
    } else if detail.contains("opaque source has no alpha convention") {
        "This image is opaque. Use source metadata for transparency."
    } else {
        "Could not prepare this conversion. Adjust the settings or choose another image."
    }
}
fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    choices: &[(T, &str)],
) {
    let ctx = ui.ctx().clone();
    ui.horizontal(|ui| {
        let caption = ui.label(t!(&ctx, label));
        let selected = choices.iter().find(|(v, _)| v == value).unwrap().1;
        egui::ComboBox::from_id_salt(label)
            .selected_text(t!(&ctx, selected))
            .show_ui(ui, |ui| {
                for (option, text) in choices {
                    ui.selectable_value(value, *option, t!(&ctx, *text));
                }
            })
            .response
            .labelled_by(caption.id);
    });
}
impl App {
    pub(crate) fn stage_native_float(&mut self, path: PathBuf) -> Result<()> {
        let family = path
            .extension()
            .and_then(|v| v.to_str())
            .and_then(Family::from_extension);
        let (family, codec, variant) = match family {
            Some(Family::Hdr) => (
                Family::Hdr,
                wire::Codec::HdrSdr8,
                wire::NATIVE_HDR_SDR_IMPORT_VARIANT,
            ),
            Some(Family::Exr) => (
                Family::Exr,
                wire::Codec::ExrSdr8,
                wire::NATIVE_EXR_SDR_IMPORT_VARIANT,
            ),
            _ => return Err(Error::new(ErrorCode::InputSchema, "Not an HDR/EXR source")),
        };
        let mut request = Request::new(
            self.format_frontend(),
            Operation::ImportConvert,
            family,
            Platform::Native,
        );
        request.extension = path.extension().and_then(|v| v.to_str());
        request.variant = Some(variant);
        match registry::admission(request) {
            registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {}
            other => other.require_allowed(request)?,
        }
        self.stage_native_float_admitted(path, codec)
    }
    // Private post-admission seam permits UI qualification while the registry is closed.
    fn stage_native_float_admitted(&mut self, path: PathBuf, codec: wire::Codec) -> Result<()> {
        if self.experience() != Experience::Studio || !codec.is_float() {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Float import requires Studio",
            ));
        }
        if self.pending_import.is_some()
            || self.has_document_drafts()
            || self.editor.has_pending_edits()
            || self.editor.has_floating_selection()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish editing before importing",
            ));
        }
        self.guard_asset_adoption(&path, None)?;
        let identity = SourceIdentity::existing_destination(&path)?.ok_or_else(|| {
            Error::new(
                ErrorCode::StorageFailed,
                "Float input must be a regular file",
            )
        })?;
        let length = identity
            .file()
            .metadata()
            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))?
            .len();
        let quote = wire::native::float_reservation_bytes(length)?;
        if quote > self.worker_budget.usage().limit {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Float import preparation allowance",
            ));
        }
        self.guard_asset_adoption(&path, Some(&identity))?;
        let preview_budget = self
            .editing
            .as_ref()
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Background import unavailable",
                )
            })?
            .metadata_budget();
        self.stage_candidate(
            Candidate::NativeCodec(Box::new(Source {
                owner: owner(self),
                stamp: self.editor.preview_stamp(),
                job: None,
                prepared: None,
                review: None,
                preview_budget,
                float: Some(Options {
                    input: path.clone(),
                    identity: identity.clone(),
                    length,
                    codec,
                    settings: Settings::default(),
                    error: None,
                }),
            })),
            path.clone(),
            String::new(),
            true,
        )?;
        self.pending_import
            .as_mut()
            .unwrap()
            .auxiliary_sources
            .push(crate::destination::SourceGuard { path, identity });
        Ok(())
    }
    pub(super) fn float_options(&self, ui: &mut egui::Ui, source: &mut Source) {
        let (changed, prepare) = source.float.as_mut().unwrap().show(ui);
        if changed {
            source.job = None;
            source.prepared = None;
            source.review = None;
            source.float.as_mut().unwrap().error = None;
        }
        if prepare {
            source.job = None;
            source.prepared = None;
            source.review = None;
            let result = self.prepare_float_preview(source);
            if let Err(error) = result {
                source.float.as_mut().unwrap().error = Some(error);
            }
        }
    }
    fn prepare_float_preview(&self, source: &mut Source) -> Result<()> {
        let options = source.float.as_mut().unwrap();
        options.error = None;
        self.guard_asset_adoption(&options.input, Some(&options.identity))?;
        let service = self.editing.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Background import unavailable",
            )
        })?;
        let residency = service.import_residency();
        let metadata = service.metadata_budget();
        let captured = source.owner;
        let input = options.input.clone();
        let identity = options.identity.clone();
        let length = options.length;
        let codec = options.codec;
        let settings = options.settings;
        source.job = Some(service.submit_import(move |cancel| {
            let lease = cancel
                .resource_lease()
                .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "Float admission missing"))?;
            Job {
                input,
                identity,
                length,
                codec,
                owner: captured,
                pixels: wire::float::PIXELS_MAX,
                deadline: Duration::from_secs(30),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: None,
            }
            .prepare_float(
                settings,
                residency,
                metadata,
                lease,
                &|| cancel.check(),
                &|| Ok(captured),
                |_, _| Ok(()),
                launch,
            )
        })?);
        Ok(())
    }
}

#[cfg(test)]
#[path = "float_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "float_installed_tests.rs"]
mod installed_tests;
