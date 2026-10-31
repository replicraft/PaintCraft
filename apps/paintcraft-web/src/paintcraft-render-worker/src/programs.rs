//! Explicit Source5 adoption and BEGIN. Old service paths stay separate.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use paintcraft_formats::export::prepared_capabilities::{
    self as capabilities, Requirements, ResolvedRequirements,
};
use protocol::programs::{self as wire, Fact, Profile};
mod begin;
pub(super) use begin::prepare;
#[derive(Debug)]
pub(super) struct State {
    pub profile: Profile,
    pub required: ResolvedRequirements,
    pub scope: BudgetScope,
    _controls: BudgetAccount,
}
pub(super) fn open(bytes: &[u8], control: &str, check: &dyn Fn() -> Result<()>) -> Result<Service> {
    let (source, profile) = wire::open(control)?;
    if bytes.len() as u64 > SOURCE_BYTES_MAX {
        return Err(resource("Prepared source exceeds64MiB"));
    }
    let extra = profile
        .bytes
        .checked_add(wire::CONTROL_BYTES)
        .ok_or_else(|| resource("Prepared source control quote"))?;
    let baseline = source
        .quote
        .checked_sub(extra)
        .ok_or_else(|| resource("Prepared source baseline"))?;
    let budget = SharedBudget::new(LINEAR_MEMORY_BYTES);
    let lease = budget.reserve(baseline)?;
    let reservation = budget.reserve_capacity(extra)?;
    let scope = reservation.scope(AllocationClass::SharedBudget);
    let mut controls = scope.account();
    controls.resize(wire::CONTROL_BYTES - capabilities::CONTROL_BYTES)?;
    check()?;
    if hex(&Sha256::digest(bytes)) != digest(field(&source.value, "sourceSha256")?)? {
        return Err(stale());
    }
    // Outer full-container SHA is distinct. ONE inner trailer witness is reused
    // for borrowed requirements and adoption; no manifest/probe rehash follows.
    let verified = pcnd::CheckedContainer::verify(bytes, check)?;
    if !(1..=6).contains(&verified.version()) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Prepared source container version",
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
            work: protocol::spatial::WORK_MAX,
        },
        &scope,
        check,
    )?;
    profile.source.matches(required.requirements(), check)?;
    let declared = wire::requirements(
        field(field(&source.value, "sourceRequirements")?, "pairs")?,
        256,
    )?;
    profile.source.matches(&declared, check)?;
    let metadata = SharedBudget::new(source.metadata_charge);
    let metadata_wire = source
        .metadata
        .checked_add(if source.metadata_charge > 0 {
            paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64
        } else {
            0
        })
        .filter(|n| *n <= paintcraft_document::source_metadata::BYTES_MAX)
        .ok_or_else(|| resource("Prepared source metadata extent"))?;
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
    profile.source.matches(&actual, check)?;
    verify_facts(&base, &source, &coverage, check)?;
    let cache = DisplayCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: CACHE_BYTES,
        preview_bytes: CACHE_BYTES,
        entries_per_domain: 128,
    })?;
    check()?;
    Ok(Service {
        source,
        base,
        candidate: None,
        cache,
        frame: None,
        generation: None,
        _lease: lease,
        workspace: None,
        programs: Some(State {
            profile,
            required,
            scope,
            _controls: controls,
        }),
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
        .ok_or_else(|| resource("Prepared source resident overflow"))
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
pub(super) fn binding(service: &Service) -> Value {
    let mut value = service
        .source
        .value
        .as_object()
        .expect("validated source object")
        .clone();
    let fact = service
        .programs
        .as_ref()
        .expect("generic source")
        .profile
        .source;
    value.insert("candidateRequirements".into(), fact.value());
    value.insert("unionRequirements".into(), fact.value());
    Value::Object(value)
}
