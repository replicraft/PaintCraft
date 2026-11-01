use super::*;
pub(super) fn open(
    bytes: &[u8],
    control: &str,
    policy: SlicePolicy,
    check: &dyn Fn() -> Result<()>,
) -> Result<NoiseService> {
    check()?;
    if bytes.len() as u64 > SOURCE_BYTES_MAX {
        return Err(resource("Source6 input exceeds64MiB"));
    }
    let q = NoiseService::control_quote()?;
    let control_bytes = wire::CONTROL_BYTES
        .checked_add(q.retained_bytes)
        .ok_or_else(|| resource("Source6 controls"))?;
    // New Source6 logical payload can NEVER consume the fixed runtime/allocator
    // headroom. Existing Source2..5 budgets and quotes are untouched.
    let payload_cap = LINEAR_MEMORY_BYTES - (32 << 20);
    let budget = SharedBudget::new(payload_cap);
    let controls_scope = budget
        .reserve_capacity(control_bytes)?
        .scope(AllocationClass::SharedBudget);
    let mut controls = controls_scope.account();
    controls.resize(
        control_bytes
            .checked_sub(capabilities::CONTROL_BYTES)
            .ok_or_else(|| resource("Source6 inventory control quote"))?,
    )?;
    // Bounded control parsing is pre-admitted; no source inventory is adopted yet.
    let (source, profile) = wire::open(control, check)?;
    let final_peak = source
        .quote
        .checked_add(q.retained_bytes)
        .and_then(|n| n.checked_add(q.extra_output_bytes))
        .ok_or_else(|| resource("Source6 peak quote"))?;
    if final_peak > payload_cap {
        return Err(resource(
            "Source6 exceeds its linear-memory payload profile",
        ));
    }
    // Source::parse prices two maximum output copies. Replace ONE by actual
    // frame storage in a reserved output scope and add the third slot required
    // by retained Rust frame -> bindgen Vec -> JS transfer copy.
    let baseline_bytes = source
        .quote
        .checked_sub(profile.workload.limits().bytes)
        .and_then(|n| n.checked_sub(wire::CONTROL_BYTES))
        .and_then(|n| n.checked_sub(OUTPUT_PIXELS_MAX * 4))
        .ok_or_else(|| resource("Source6 baseline partition"))?;
    let baseline = budget.reserve(baseline_bytes)?;
    let workspace = budget
        .reserve_capacity(profile.workload.limits().bytes)?
        .scope(AllocationClass::SharedBudget);
    let output = budget
        .reserve_capacity(2 * OUTPUT_PIXELS_MAX * 4)?
        .scope(AllocationClass::SharedBudget);
    check()?;
    // One combined byte traversal computes outer SHA and authenticates the
    // inner trailer. This witness is reused by requirements and adopting read.
    let (verified, source_hash) = pcnd::CheckedContainer::verify_with_source_hash(bytes, check)?;
    if hex(&source_hash) != digest(field(&source.value, "sourceSha256")?)? {
        return Err(stale());
    }
    if !(1..=6).contains(&verified.version()) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source6 container version",
        ));
    }
    let required = Requirements::from_source(
        &verified,
        pcnd::RequirementsLimits {
            layers: source.layers,
            tiles: source.resident / paintcraft_raster::RasterLayer::TILE_BYTES as u64,
            metadata_bytes: 8 << 20,
            coverage: paintcraft_raster::coverage_mask::Limits {
                bands: source.masks.bands as usize,
                runs: source.masks.runs as usize,
                ..Default::default()
            },
            // Parsing/metadata work stays on the old finite base bound, not Noise.
            work: 268_435_456,
        },
        &controls_scope,
        check,
    )?;
    required
        .requirements()
        .validate_contract(RegistryContract::ScopedNoise2, check)?;
    profile
        .source
        .matches(required.requirements(), profile.workload, check)?;
    let declared = wire::requirements(
        field(field(&source.value, "sourceRequirements")?, "pairs")?,
        256,
    )?;
    profile.source.matches(&declared, profile.workload, check)?;
    let metadata = SharedBudget::new(source.metadata_charge);
    let metadata_wire = source
        .metadata
        .checked_add(if source.metadata_charge > 0 {
            paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64
        } else {
            0
        })
        .filter(|n| *n <= paintcraft_document::source_metadata::BYTES_MAX)
        .ok_or_else(|| resource("Source6 metadata extent"))?;
    let coverage = SharedBudget::new(source.masks.decode_bytes);
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
                bytes: source.masks.decode_bytes,
                budget: &coverage,
                limits: paintcraft_raster::coverage_mask::Limits {
                    bands: source.masks.bands as usize,
                    runs: source.masks.runs as usize,
                    ..Default::default()
                },
            },
        },
        check,
    )?;
    let actual = Requirements::capture(&base, check)?;
    actual.validate_contract(RegistryContract::ScopedNoise2, check)?;
    profile.source.matches(&actual, profile.workload, check)?;
    verify_facts(&base, &source, &coverage, check)?;
    check()?;
    Ok(NoiseService {
        state: Box::new(State {
            source,
            base,
            candidate: None,
            frame: None,
            last_binding: None,
            generation: None,
            profile,
            required,
            policy,
            workspace,
            output,
            _controls_scope: controls_scope,
        }),
        baseline: Shared::new(baseline),
        controls: Shared::new(controls),
    })
}
fn verify_facts(
    base: &Document,
    source: &Source,
    coverage: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let resident = base.layers.values().try_fold(0u64, |n, l| {
        check()?;
        n.checked_add(
            l.raster.resident_tiles() as u64 * paintcraft_raster::RasterLayer::TILE_BYTES as u64,
        )
        .ok_or_else(|| resource("Source6 resident overflow"))
    })?;
    let profile = base
        .source_metadata
        .as_ref()
        .and_then(|m| m.profile())
        .map_or(0, |p| p.len() as u64);
    let retained = base
        .source_metadata
        .as_ref()
        .map_or(0, |m| m.storage_usage().1);
    let masks = pcnd::coverage_facts(base, check)?;
    let inventory = filter_bake::inspect(base, check)?;
    if masks.masks != source.masks.masks
        || masks.bands != source.masks.bands
        || masks.runs != source.masks.runs
        || masks.decode_bytes > source.masks.decode_bytes
        || coverage.usage().reserved > source.masks.decode_bytes
        || base.state_id != source.state
        || base.document_id != source.document
        || base.canvas != source.size
        || base.layers.len() != source.layers
        || base.roots.len() != source.layers
        || resident > source.resident
        || profile != source.metadata
        || retained > source.metadata_charge
        || hex(&inventory.digest()) != digest(field(&source.value, "filterDigest")?)?
    {
        return Err(stale());
    }
    Ok(())
}
