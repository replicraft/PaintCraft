//! Persistent private render service. No Session or History is constructed.
//! Staging and each output patch are synchronous; cancellation is acknowledged
//! between patches. Large sparse reduction may delay one patch (not a time cap).
mod generic_frame;
mod programs;
pub mod protocol;
mod spatial;
#[cfg(target_arch = "wasm32")]
mod wasm;
use paintcraft_commands::{Batch, FilterOp, FilterRef, LayerRef, Op};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::Value,
    policy::hex,
    resources::{AllocationClass, BudgetLease, BudgetScope, SharedBudget},
};
use paintcraft_document::Document;
use paintcraft_formats::{export::filter_bake, pcnd};
use paintcraft_render::{
    cache::{CacheLimits, Domain},
    display::{DisplayCache, DisplayRequest},
};
use protocol::*;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug)]
struct Frame {
    binding: Value,
    request: DisplayRequest,
    pixels: Vec<u8>,
    column: u32,
    row: u32,
    spatial: Option<spatial::NativeFrame>,
    programs: Option<generic_frame::GenericFrame>,
}
#[derive(Debug)]
pub struct Service {
    source: Source,
    base: Document,
    candidate: Option<Document>,
    cache: DisplayCache,
    frame: Option<Frame>,
    generation: Option<u64>,
    _lease: BudgetLease,
    workspace: Option<BudgetScope>,
    programs: Option<programs::State>,
}
impl Service {
    pub fn open(bytes: &[u8], control: &str) -> Result<Self> {
        let source = Source::parse(parse(control)?)?;
        if bytes.len() as u64 > SOURCE_BYTES_MAX {
            return Err(resource("Filter preview source exceeds64MiB"));
        }
        let budget = SharedBudget::new(LINEAR_MEMORY_BYTES);
        let lease = budget.reserve(source.quote)?;
        if hex(&Sha256::digest(bytes)) != digest(field(&source.value, "sourceSha256")?)? {
            return Err(stale());
        }
        // Source authentication above already scanned these bytes. Inspect only
        // the fixed magic here; probing would add another full-source hash pass.
        if bytes.len() >= 8 && &bytes[..4] == b"PCND" && &bytes[5..8] == b"\r\n\x1a" {
            let version = bytes[4];
            if source.spatial.is_none() && version == 5 {
                return Err(protocol::spatial::refused());
            }
            let maximum = if source
                .spatial
                .is_some_and(|profile| profile.catalog_count.is_some())
            {
                6
            } else if source.spatial.is_some() {
                5
            } else {
                4
            };
            if !(1..=maximum).contains(&version) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    if maximum == 4 {
                        "Preview source2 admits only PCND1 through PCND4"
                    } else if maximum == 5 {
                        "Preview source3 admits only PCND1 through PCND5"
                    } else {
                        "Preview source4 admits only PCND1 through PCND6"
                    },
                ));
            }
        }
        if source
            .spatial
            .is_some_and(|profile| profile.catalog_count.is_some())
        {
            for requirement in pcnd::catalog_manifest(bytes)? {
                if !protocol::spatial::catalog_kind_allowed(&requirement.profile_id)
                    || requirement.codec_id != "params-v1"
                {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Preview source4 catalog capability is closed",
                    ));
                }
            }
        }
        let workspace = source
            .spatial
            .map(|profile| {
                SharedBudget::new(profile.bytes)
                    .reserve_capacity(profile.bytes)
                    .map(|reservation| reservation.scope(AllocationClass::SharedBudget))
            })
            .transpose()?;
        let metadata = SharedBudget::new(source.metadata_charge);
        // The protocol echoes profile payload length. PCND's reader allowance
        // includes the fixed source descriptor even for resolution-only sources.
        let metadata_wire = source
            .metadata
            .checked_add(if source.metadata_charge > 0 {
                paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64
            } else {
                0
            })
            .filter(|bytes| *bytes <= paintcraft_document::source_metadata::BYTES_MAX)
            .ok_or_else(|| resource("Filter preview source metadata extent"))?;
        let coverage = SharedBudget::new(source.masks.decode_bytes);
        let base = pcnd::read_with_admission_checked(
            bytes,
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
                        ..paintcraft_raster::coverage_mask::Limits::default()
                    },
                },
            },
            &|| Ok(()),
        )?;
        let count = protocol::spatial::stored(&base);
        let catalog_count = protocol::spatial::stored_catalog(&base);
        if let Some(expected) = source.spatial.and_then(|profile| profile.catalog_count) {
            if expected != catalog_count {
                return Err(stale());
            }
        } else if catalog_count != 0 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Catalog filters require preview source4",
            ));
        }
        if source.spatial.is_some_and(|profile| profile.count != count) {
            return Err(stale());
        }
        if source.spatial.is_none() && count != 0 {
            return Err(protocol::spatial::refused());
        }
        let resident = base.layers.values().try_fold(0u64, |n, l| {
            n.checked_add(
                l.raster.resident_tiles() as u64
                    * paintcraft_raster::RasterLayer::TILE_BYTES as u64,
            )
            .ok_or_else(|| resource("Resident overflow"))
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
        let inventory = filter_bake::inspect(&base, &|| Ok(()))?;
        let mask_facts = pcnd::coverage_facts(&base, &|| Ok(()))?;
        if mask_facts.masks != source.masks.masks
            || mask_facts.bands != source.masks.bands
            || mask_facts.runs != source.masks.runs
            || mask_facts.decode_bytes > source.masks.decode_bytes
            || coverage.usage().reserved > source.masks.decode_bytes
        {
            return Err(stale());
        }
        if base.state_id != source.state
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
        let cache = DisplayCache::new(CacheLimits {
            tile_edge: 32,
            committed_bytes: CACHE_BYTES,
            preview_bytes: CACHE_BYTES,
            entries_per_domain: 128,
        })?;
        Ok(Self {
            source,
            base,
            candidate: None,
            cache,
            frame: None,
            generation: None,
            _lease: lease,
            workspace,
            programs: None,
        })
    }
    pub fn open_programs(bytes: &[u8], control: &str) -> Result<Self> {
        programs::open(bytes, control, &|| Ok(()))
    }
    pub fn capabilities(control: &str) -> Result<Value> {
        protocol::programs::hello(control)
    }
    pub fn context(&self) -> Value {
        if self.programs.is_some() {
            return programs::binding(self);
        }
        self.source.value.clone()
    }
    pub fn prepare(&mut self, control: &str) -> Result<Value> {
        if self.programs.is_some() {
            return programs::prepare(self, control);
        }
        let value = parse(control)?;
        object(
            &value,
            &[
                "owner",
                "draft",
                "sourceSha256",
                "generation",
                "operation",
                "target",
                "view",
                "render",
            ],
        )?;
        for name in ["owner", "draft", "sourceSha256"] {
            if field(&value, name)? != field(&self.source.value, name)? {
                return Err(stale());
            }
        }
        let generation = decimal(field(&value, "generation")?)?;
        if self.generation.is_some_and(|old| generation <= old) {
            return Err(stale());
        }
        let view = View::parse(field(&value, "view")?)?;
        let render = field(&value, "render")?
            .as_bool()
            .ok_or_else(|| bad("Expected render boolean"))?;
        self.retire();
        self.generation = Some(generation);
        let operation = field(&value, "operation")?;
        let target = field(&value, "target")?;
        if *operation != Value::Null {
            let target = uuid(target)?;
            let name = field(operation, "op")?
                .as_str()
                .ok_or_else(|| bad("Filter operation needs a name"))?;
            // A private parameter update cannot allocate or publish coverage.
            // Refuse before even decoding an A8 command input.
            if name.starts_with("filter.mask.") {
                return Err(mask_preview_error());
            }
            let kind = operation
                .get("descriptor")
                .and_then(|descriptor| descriptor.get("kind"))
                .and_then(Value::as_str);
            if self
                .source
                .spatial
                .is_some_and(|profile| profile.catalog_count.is_some())
                && kind.is_some_and(|kind| !protocol::spatial::catalog_kind_allowed(kind))
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Preview source4 catalog capability is closed",
                ));
            }
            if self.source.spatial.is_none() && kind == Some("box-blur-v1") {
                return Err(protocol::spatial::refused());
            }
            if kind == Some("levels-rgb-v1")
                && self
                    .source
                    .spatial
                    .is_none_or(|profile| profile.catalog_count.is_none())
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Catalog filter preview requires prepared-program admission",
                ));
            }
            let schema = Batch::schema_for_operation_fields([(name, kind)]);
            // Closed Source2/3 cannot inherit a catalog execution route through
            // automatic batch selection, including disabled/identity intents.
            // Keep Source4's earlier exact list and legacy Levels diagnostic.
            if schema == paintcraft_commands::CATALOG_BATCH_SCHEMA
                && self
                    .source
                    .spatial
                    .is_none_or(|p| p.catalog_count.is_none())
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Catalog filter preview requires generic program admission",
                ));
            }
            let batch = Batch::from_generated_value(Value::object([
                ("schema", Value::str(schema)),
                ("atomic", Value::Bool(true)),
                ("policy", Value::str(paintcraft_document::POLICY)),
                (
                    "base",
                    Value::object([
                        ("state_id", Value::str(self.base.state_id.to_string())),
                        (
                            "publication_epoch",
                            field(field(&self.source.value, "owner")?, "epoch")?.clone(),
                        ),
                    ]),
                ),
                ("operations", Value::Array(vec![operation.clone()])),
            ]))?;
            let Some(Op::Filter(op)) = batch.operations.first() else {
                return Err(bad("Only typed filter operations can be previewed"));
            };
            stable_target(op, target)?;
            // No Session/preparation/history cloning or publication. Original
            // raster payloads stay shared; execute owns private metadata copies.
            self.candidate = Some(
                paintcraft_editor::execute::execute_with_admission_checked(
                    &self.base,
                    paintcraft_raster::Selection::Inactive,
                    &batch,
                    paintcraft_raster::Admission::new(0),
                    &|| Ok(()),
                )?
                .doc,
            );
        } else if *target != Value::Null {
            return Err(bad("Base rendering has no target"));
        }
        let doc = self.candidate.as_ref().unwrap_or(&self.base);
        let request = DisplayRequest::new(doc, view.rect, view.level)?;
        let [width, height] = request.size();
        let count = u64::from(width) * u64::from(height);
        if count > OUTPUT_PIXELS_MAX {
            return Err(resource("Filter preview output exceeds4MP"));
        }
        let rect = request.region();
        let native =
            if let (Some(profile), Some(scope)) = (self.source.spatial, self.workspace.clone()) {
                spatial::NativeFrame::prepare(doc, request, profile, scope, render)?
            } else {
                None
            };
        let binding = Value::object([
            (
                "kind",
                Value::str(if render { "frame" } else { "prepared" }),
            ),
            ("generation", Value::str(generation.to_string())),
            ("owner", field(&value, "owner")?.clone()),
            ("draft", field(&value, "draft")?.clone()),
            ("sourceSha256", field(&value, "sourceSha256")?.clone()),
            ("target", target.clone()),
            ("view", view.value),
            ("width", Value::Int(i64::from(width))),
            ("height", Value::Int(i64::from(height))),
            (
                "region",
                Value::Array(vec![
                    Value::Int(rect.x0),
                    Value::Int(rect.y0),
                    Value::Int(rect.width()),
                    Value::Int(rect.height()),
                ]),
            ),
            ("level", Value::Int(i64::from(view.level))),
        ]);
        let mut binding = binding.as_object().expect("binding object").clone();
        binding.insert(
            "operationSha256".into(),
            Value::str(hex(&Sha256::digest(
                paintcraft_core::types::json::canonical(operation),
            ))),
        );
        let binding = Value::Object(binding);
        if render {
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(count as usize * 4)
                .map_err(|_| resource("Filter preview output allocation"))?;
            pixels.resize(count as usize * 4, 0);
            self.frame = Some(Frame {
                binding: binding.clone(),
                request,
                pixels,
                column: 0,
                row: 0,
                spatial: native,
                programs: None,
            });
        }
        Ok(binding)
    }
    /// One output patch; this is not a maximum wall-clock execution duration.
    pub fn step(&mut self) -> Result<bool> {
        let f = self
            .frame
            .as_mut()
            .ok_or_else(|| bad("No filter frame active"))?;
        if let Some(mut native) = f.programs.take() {
            let doc = self.candidate.as_ref().unwrap_or(&self.base);
            let result = native.step(doc, f);
            f.programs = Some(native);
            return result;
        }
        if let Some(mut native) = f.spatial.take() {
            let doc = self.candidate.as_ref().unwrap_or(&self.base);
            let result = native.step(doc, f);
            f.spatial = Some(native);
            return result;
        }
        let [width, height] = f.request.size();
        if f.row >= height {
            return Ok(true);
        }
        let step = i64::from(f.request.step());
        let region = f.request.region();
        let x = region.x0 + i64::from(f.column) * step;
        let y = region.y0 + i64::from(f.row) * step;
        let rect = paintcraft_core::types::Rect {
            x0: x,
            y0: y,
            x1: (x + 32 * step).min(region.x1),
            y1: (y + 32 * step).min(region.y1),
        };
        let doc = self.candidate.as_ref().unwrap_or(&self.base);
        let request = DisplayRequest::new(doc, rect, f.request.level())?;
        let patch = self.cache.render(
            doc,
            request,
            if self.candidate.is_some() {
                Domain::Preview
            } else {
                Domain::Committed
            },
            || Ok(()),
        )?;
        let [w, h] = request.size();
        for row in 0..h as usize {
            let dst = ((f.row as usize + row) * width as usize + f.column as usize) * 4;
            f.pixels[dst..dst + w as usize * 4]
                .copy_from_slice(&patch.pixels[row * w as usize * 4..(row + 1) * w as usize * 4]);
        }
        f.column += 32;
        if f.column >= width {
            f.column = 0;
            f.row += 32;
        }
        Ok(f.row >= height)
    }
    pub fn take_frame(&mut self) -> Result<(Value, Vec<u8>)> {
        let f = self
            .frame
            .as_ref()
            .ok_or_else(|| bad("No completed frame"))?;
        if f.row < f.request.size()[1] {
            return Err(bad("Frame is incomplete"));
        }
        let f = self.frame.take().expect("checked frame");
        Ok((f.binding, f.pixels))
    }
    pub fn retire(&mut self) {
        self.frame = None;
        self.candidate = None;
        self.cache.clear_preview();
    }
}
fn mask_preview_error() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Mask edits require separately admitted editor publication",
    )
}
fn stable_target(op: &FilterOp, target: Uuid) -> Result<()> {
    let (layer, filter) = match op {
        FilterOp::Mask { .. } => return Err(mask_preview_error()),
        FilterOp::Add { layer, .. } => (layer, None),
        FilterOp::Update { layer, filter, .. }
        | FilterOp::SetEnabled { layer, filter, .. }
        | FilterOp::Reorder { layer, filter, .. }
        | FilterOp::Remove { layer, filter }
        | FilterOp::Duplicate { layer, filter, .. } => (layer, Some(filter)),
    };
    if *layer != LayerRef::Id(target) || filter.is_some_and(|f| !matches!(f, FilterRef::Id(_))) {
        return Err(bad("Host filter previews require stable UUID references"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
