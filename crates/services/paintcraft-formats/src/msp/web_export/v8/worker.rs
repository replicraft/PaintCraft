use super::*;
pub fn execute(bytes: &[u8], version: &str, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    if bytes.len() < REQUEST_HEADER || bytes.len() > REQUEST_HEADER + SOURCE_BYTES_MAX {
        return Err(resource("Browser MSP request allowance"));
    }
    let request = Header::decode(&bytes[..REQUEST_HEADER])?;
    let result = (|| {
        // This is an isolated job boundary, not a nested parent upgrade. Refuse
        // installed callers before allocating a separate source/payload realm.
        let absent = std::cell::Cell::new(false);
        let current = capture_noise_or_current(
            || {
                absent.set(true);
                Err(failed("MSP8 no installed caller"))
            },
            request.workload.limits(),
        );
        if !absent.get() {
            current?;
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "MSP8 isolated endpoint cannot replace an installed render job",
            ));
        }
        let source = &bytes[REQUEST_HEADER..];
        request.check_source_checked(source, check)?;

        // The existing parser inventories/checks tiles before inflating any
        // pixel payload. Full padded tiles, not logical edge samples, are priced.
        // Price retained profile/control payload before the reader can allocate.
        let controls = export::prepared_capabilities::CONTROL_BYTES
            .checked_add(control_bytes()?)
            .ok_or_else(|| resource("MSP8 controls"))?;
        let parser_base = request
            .masks
            .pre_read_quote(export::web_snapshot::pre_read_quote(
                source.len(),
                NOISE_SOURCE_LIMITS,
            )?)?;
        request.metadata.check_capture_quote(
            parser_base
                .checked_add(controls)
                .ok_or_else(|| resource("Generic parser control overflow"))?,
            WORKER_PAYLOAD_BYTES_MAX,
        )?;
        let parser_base = parser_base
            .checked_add(request.metadata.retained_charge())
            .ok_or_else(|| resource("Generic parser baseline overflow"))?;
        let pre_read = parser_base
            .checked_add(controls)
            .ok_or_else(|| resource("Generic pre-read overflow"))?;
        let profile_budget =
            paintcraft_core::types::resources::SharedBudget::new(WORKER_PAYLOAD_BYTES_MAX);
        let reservation =
            profile_budget.reserve_capacity(pre_read.max(request.programs.phase_payload(
                request.spatial,
                request.workload,
                encode_extra(request.version, request.size)?,
                WORKER_PAYLOAD_BYTES_MAX,
            )?))?;
        let scope =
            reservation.scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
        let mut baseline = scope.account();
        baseline.resize(parser_base)?;
        // Reuse ONE actual inner-integrity witness for borrowed registry resolution
        // and typed adoption; outer SourceSHA is separate and already checked.
        let verified = pcnd::CheckedContainer::verify(source, check)?;
        let actual_resident = verified
            .snapshot_tile_records_checked(check)?
            .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
            .filter(|n| *n <= RESIDENT_BYTES_MAX)
            .ok_or_else(|| resource("MSP8 actual resident records"))?;
        let requirements = export::prepared_capabilities::Requirements::from_source(
            &verified,
            pcnd::RequirementsLimits {
                layers: LAYERS_MAX,
                tiles: RESIDENT_BYTES_MAX / paintcraft_raster::RasterLayer::TILE_BYTES as u64,
                metadata_bytes: 8 << 20,
                coverage: Default::default(),
                work: export::SPATIAL_WORK_MAX,
            },
            &scope,
            check,
        )?;
        request.programs.validate_requirements(
            requirements.requirements(),
            request.workload,
            check,
        )?;
        drop(requirements);
        baseline.resize(pre_read)?;
        let metadata_budget = paintcraft_core::types::resources::SharedBudget::new(
            request.metadata.retained_charge(),
        );
        let mask_budget =
            paintcraft_core::types::resources::SharedBudget::new(request.masks.decode_bytes());
        let doc = verified.read_with_admission_checked(
            PIXELS_MAX,
            RESIDENT_BYTES_MAX / 4,
            LAYERS_MAX,
            request.masks.admission(
                pcnd::MetadataAdmission {
                    bytes: u64::from(request.metadata.wire_bytes()),
                    budget: &metadata_budget,
                },
                &mask_budget,
            ),
            check,
        )?;

        request.masks.validate_decoded(&doc)?;
        if doc.state_id != request.owner.state || doc.canvas != request.size {
            return Err(failed(
                "Browser snapshot owner/geometry disagrees with request",
            ));
        }
        let policy = request.metadata.validate_document(
            &doc,
            export::metadata_policy::Target::new(
                crate::registry::Family::Msp,
                crate::registry::Operation::ExportFlat,
            ),
        )?;
        // Inventory is descriptive: only the independent request intent can
        // mint a copy policy after the exact immutable snapshot is checked.
        let filters = export::filter_bake::inspect(&doc, check)?;
        if filters.digest() != request.filter_digest
            || filters.has_filters() != (request.source_flags & 4 != 0)
            || filters.filter_count()
                > (LAYERS_MAX * paintcraft_document::filters::FILTERS_MAX) as u64
            || filters.filter_storage_bytes()
                > export::web_snapshot::filter_storage_ceiling(LAYERS_MAX)?
        {
            return Err(failed("Browser filter inventory disagrees with request"));
        }
        let target = export::metadata_policy::Target::new(
            crate::registry::Family::Msp,
            crate::registry::Operation::ExportFlat,
        );
        let bake = export::filter_bake::Policy::capture(
            &doc,
            target,
            if request.bake_intent == 1 {
                export::filter_bake::Intent::Bake
            } else {
                export::filter_bake::Intent::Refuse
            },
            check,
        )?;
        bake.validate(&doc, target, check)?;
        let selected_version = request.version;
        let plan = NoiseSourcePlan::capture_with_policies_checked(
            &doc,
            selected_version,
            export::CopyPolicies {
                source_metadata: policy,
                filter_bake: bake,
            },
            request.workload,
            check,
        )?;
        if plan.source_state() != request.owner.state
            || plan.size() != request.size
            || plan.version() != request.version
            || plan.source_flags() != request.source_flags
        {
            return Err(failed("Browser MSP snapshot disagrees with request"));
        }
        let (region, programs) = plan.finalize(source.len(), actual_resident)?;
        if region != request.spatial || programs != request.programs {
            return Err(failed("MSP8 independently decoded snapshot facts differ"));
        }
        let phase = request.programs.phase_payload(
            request.spatial,
            request.workload,
            0,
            WORKER_PAYLOAD_BYTES_MAX,
        )?;
        let render_baseline = phase
            .checked_sub(request.programs.workspace_peak()?)
            .ok_or_else(|| resource("MSP8 render phase"))?;
        baseline.resize(render_baseline)?;
        let baseline = std::cell::RefCell::new(baseline);
        let admission = NoiseAdmission::new_root(
            scope.clone(),
            NoiseJobPolicy::BrowserV1,
            request.workload.limits(),
        )?;
        admission.check_available(request.programs.work())?;
        let admission = admission.prepare_document(&doc, check)?;
        with_noise_workspace(admission, || {
            let artifact = export::msp::prepare_document_with_policies(
                &doc,
                selected_version,
                &LOSSES.map(str::to_owned),
                |layout, resident| {
                    if layout.version() != selected_version
                        || (layout.width(), layout.height()) != request.size
                        || resident > RESIDENT_BYTES_MAX
                    {
                        return Err(resource("MSP8 render extent"));
                    }
                    check()
                },
                |plan| {
                    if plan.layout().version() != selected_version
                        || plan.encoded_bytes() > ENCODED_BYTES_MAX
                    {
                        return Err(resource("MSP8 encoding allowance"));
                    }
                    let extra = plan.additional_bytes() as u64;
                    request.programs.phase_payload(
                        request.spatial,
                        request.workload,
                        extra,
                        WORKER_PAYLOAD_BYTES_MAX,
                    )?;
                    // Appearance workspaces are gone. Keep retained prepared
                    // owners/counter in the SAME root while packed/output live.
                    baseline.borrow_mut().resize(
                        render_baseline
                            .checked_add(extra)
                            .ok_or_else(|| resource("MSP8 encoded phase"))?,
                    )?;
                    check()
                },
                render_frame,
                check,
                export::CopyPolicies {
                    source_metadata: policy,
                    filter_bake: bake,
                },
            )?;
            // Actual packed decode reuses the admitted packed slot; composite
            // is gone so decoded RGBA reuses its slot. No rerender or new realm.
            let bitmap = crate::msp::decode(artifact.bytes(), check)?;
            if bitmap.header().version() != selected_version {
                return Err(failed("MSP8 encoded version"));
            }
            let rgba = bitmap.rgba(check)?;
            let size = preview_size(request.size);
            let mut preview = Vec::new();
            preview
                .try_reserve_exact((size.0 * size.1 * 4) as usize)
                .map_err(|_| resource("Browser MSP preview allocation"))?;
            for y in 0..size.1 {
                check()?;
                for x in 0..size.0 {
                    let sx = u64::from(x) * u64::from(request.size.0) / u64::from(size.0);
                    let sy = u64::from(y) * u64::from(request.size.1) / u64::from(size.1);
                    let at = (sy * u64::from(request.size.0) + sx) as usize * 4;
                    preview.extend_from_slice(&rgba[at..at + 4]);
                }
            }
            drop(rgba);
            drop(bitmap);
            check()?;
            response::success(&request, &artifact, &preview, size, version)
        })
    })();
    match result {
        Ok(bytes) => Ok(bytes),
        Err(error) => response::refused(&request, &error, version),
    }
}

fn render_frame(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    paintcraft_render::render_cancellable(doc, check)
}
