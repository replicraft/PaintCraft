use super::*;
use paintcraft_render::noise_work::NoiseLimits;
pub(super) fn open(
    bytes: &[u8],
    control: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<LayerAppearanceService> {
    check()?;
    if bytes.len() as u64 > SOURCE_BYTES_MAX {
        return Err(resource("Source7 source exceeds64MiB"));
    }
    let q = LayerAppearanceService::control_quote()?;
    let payload_cap = LINEAR_MEMORY_BYTES - (32 << 20);
    let budget = SharedBudget::new(payload_cap);
    // Variable Formats scratch is reserved separately once layers are parsed.
    // Remove that fixed part here so the final source_quote is not double-used.
    let fixed = LayerAppearanceService::fixed_control_bytes()?
        .checked_sub(coverage7::CONTROL_BYTES)
        .ok_or_else(|| resource("Source7 control partition"))?;
    let controls_scope = budget
        .reserve_capacity(fixed)?
        .scope(AllocationClass::SharedBudget);
    let mut controls = controls_scope.account();
    controls.resize(fixed)?;
    let (source, profile) = wire::open(control, check)?;
    let total = profile.source_quote(&source)?;
    let final_peak = total
        .checked_add(q.retained_bytes)
        .and_then(|n| n.checked_add(q.extra_output_bytes))
        .ok_or_else(|| resource("Source7 final peak"))?;
    if final_peak > payload_cap {
        return Err(resource("Source7 payload exceeds declared headroom"));
    }
    let transient = coverage7::transient_quote(source.layers)?;
    let inventory = budget
        .reserve_capacity(transient)?
        .scope(AllocationClass::SharedBudget);
    let workspace = budget
        .reserve_capacity(profile.admission.workspace_bytes)?
        .scope(AllocationClass::SharedBudget);
    let output = budget
        .reserve_capacity(2 * OUTPUT_PIXELS_MAX * 4)?
        .scope(AllocationClass::SharedBudget);
    let baseline_bytes = total
        .checked_sub(profile.admission.workspace_bytes)
        .and_then(|n| n.checked_sub(wire::CONTROL_BYTES))
        .and_then(|n| n.checked_sub(transient.checked_sub(coverage7::CONTROL_BYTES)?))
        .and_then(|n| n.checked_sub(OUTPUT_PIXELS_MAX * 4))
        .ok_or_else(|| resource("Source7 baseline partition"))?;
    let mut baseline = budget.account(AllocationClass::SharedBudget);
    baseline.resize(baseline_bytes)?;
    let metadata_stage = budget.scope(AllocationClass::SharedBudget);
    let mode = match profile.admission.mode {
        Mode7::Base => Mode::Base,
        Mode7::Noise => Mode::Noise,
    };
    let job = OpenJob::capture(
        mode,
        NoiseLimits {
            bytes: profile.admission.workspace_bytes,
            base_work: profile.admission.base_work,
            noise_work: profile.admission.noise_work,
        },
        workspace.clone(),
    )?;
    // Parsing was bounded/pre-admitted before mode was known. Charge its complete
    // structural extent now, before authentication/probe/adoption or publication.
    job.spend_control(control::parse_quote(control.len())?)?;
    job.spend_control(metadata::admission_work_quote())?;
    let admission_sha256 = profile.admission.digest()?;
    // One combined verification traversal; price two named SHA byte intakes
    // (inner body <= source length, outer whole source == source length).
    // Raw cancellation callback below must NOT debit these bytes a second time.
    job.spend_control(
        (bytes.len() as u64)
            .checked_mul(2)
            .ok_or_else(|| resource("Source7 authentication work"))?,
    )?;
    let (verified, source_sha) = pcnd::CheckedContainer::verify_with_source_hash(bytes, check)?;
    if verified.version() != 7 {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 requires PCND7",
        ));
    }
    if hex(&source_sha) != digest(field(&source.value, "sourceSha256")?)? {
        return Err(stale());
    }
    let facts = profile.aggregate_coverage(&source)?;
    let limits = pcnd::RequirementsLimits {
        layers: source.layers,
        tiles: source.resident / paintcraft_raster::RasterLayer::TILE_BYTES as u64,
        metadata_bytes: 8 << 20,
        coverage: paintcraft_raster::coverage_mask::Limits {
            bands: facts.bands as usize,
            runs: facts.runs as usize,
            ..Default::default()
        },
        work: profile.admission.base_work,
    };
    let spend = |n| job.spend_control(n);
    let probe = verified.probe_layer_appearance_checked(limits, &inventory, check, &spend)?;
    profile
        .admission
        .validate_requirements(probe.requirements())?;
    let source_fact = control::fact(
        probe.requirements(),
        profile.admission,
        probe.inventory(),
        &job,
        check,
    )?;
    if source_fact != profile.source {
        return Err(stale());
    }
    let metadata = SharedBudget::new(source.metadata_charge);
    let metadata_wire = source
        .metadata
        .checked_add(if source.metadata_charge != 0 {
            paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64
        } else {
            0
        })
        .filter(|n| *n <= paintcraft_document::source_metadata::BYTES_MAX)
        .ok_or_else(|| resource("Source7 metadata extent"))?;
    if probe.wire_metadata_bytes() > metadata_wire {
        return Err(stale());
    }
    let coverage = SharedBudget::new(facts.decode_bytes);
    let (base, adopted) = {
        let charged_check = control::checked(&job, check);
        let base = verified.read_with_admission_checked(
            u64::MAX,
            source.resident / 4,
            source.layers,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: metadata_wire,
                    budget: &metadata,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: facts.decode_bytes,
                    budget: &coverage,
                    limits: paintcraft_raster::coverage_mask::Limits {
                        bands: facts.bands as usize,
                        runs: facts.runs as usize,
                        ..Default::default()
                    },
                },
            },
            &charged_check,
        )?;
        let adopted = probe.finish_adoption_checked(&base, &inventory, check, &spend)?;
        // Provenance is this actual fresh PCND decoder invocation, not a capacity
        // equality check on an arbitrary live Document. No mutation occurred since.
        let actual_metadata = paintcraft_editor::execute::metadata_clone::retained_quote_checked(
            &base, check, &spend,
        )?;
        let old_metadata = (source.layers as u64)
            .checked_mul(
                (std::mem::size_of::<paintcraft_document::Layer>()
                    + 256 * std::mem::size_of::<paintcraft_document::FilterInstance>())
                    as u64,
            )
            .and_then(|n| n.checked_mul(3))
            .ok_or_else(|| resource("Source7 legacy metadata projection"))?;
        let reconciled = baseline_bytes
            .checked_sub(old_metadata)
            .and_then(|n| n.checked_add(actual_metadata))
            .ok_or_else(|| resource("Source7 actual metadata reconciliation"))?;
        // In-place resize: never release all charge while source storage still lives.
        // Any growth is admitted under the SAME224MiB budget before publication.
        baseline.resize(reconciled)?;
        job.validate_document(&base, &charged_check)?;
        let filter_digest = coverage7::filter_digest_checked(&base, &inventory, check, &spend)?;
        let mut resident = 0u64;
        for l in base.layers.values() {
            charged_check()?;
            resident = resident
                .checked_add(
                    (l.raster.resident_tiles() as u64)
                        .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
                        .ok_or_else(|| resource("Source7 actual resident"))?,
                )
                .ok_or_else(|| resource("Source7 actual resident"))?;
        }
        let metadata_payload = base
            .source_metadata
            .as_ref()
            .and_then(|m| m.profile())
            .map_or(0, |p| p.len() as u64);
        let metadata_retained = base
            .source_metadata
            .as_ref()
            .map_or(0, |m| m.storage_usage().1);
        if base.state_id != source.state
            || base.document_id != source.document
            || base.canvas != source.size
            || base.layers.len() != source.layers
            || base.roots.len() != source.layers
            || resident > source.resident
            || metadata_payload != source.metadata
            || metadata_retained > source.metadata_charge
            || coverage.usage().reserved > facts.decode_bytes
            || adopted.source_inventory().filter_coverage()? != source.masks
            || hex(&filter_digest) != digest(field(&source.value, "filterDigest")?)?
        {
            return Err(stale());
        }
        (base, adopted)
    };
    check()?;
    Ok(LayerAppearanceService {
        state: Box::new(State {
            source,
            base,
            candidate: None,
            frame: None,
            last_binding: None,
            last_copy_admission: None,
            generation: None,
            profile,
            adopted,
            admission_sha256,
            job,
            _workspace: workspace,
            output,
            inventory,
            metadata_stage,
        }),
        baseline: Shared::new(baseline),
        controls: Shared::new(controls),
    })
}
