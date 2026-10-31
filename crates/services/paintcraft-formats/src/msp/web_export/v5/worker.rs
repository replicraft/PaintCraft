use super::*;
/// Complete request only; the existing browser service owns scheduling/limits.
pub fn execute(bytes: &[u8], version: &str, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    if bytes.len() < REQUEST_HEADER || bytes.len() > REQUEST_HEADER + SOURCE_BYTES_MAX {
        return Err(resource("Browser MSP2 request allowance"));
    }
    let request = Header::decode(&bytes[..REQUEST_HEADER])?;
    let result = (|| {
        let source = &bytes[REQUEST_HEADER..];
        request.check_source(source)?;

        // Price retained profile/control payload before the reader can allocate.
        request.metadata.check_capture_quote(
            request
                .masks
                .pre_read_quote(export::web_snapshot::pre_read_quote(
                    source.len(),
                    SOURCE_LIMITS,
                )?)?,
            WORKER_PAYLOAD_BYTES_MAX,
        )?;
        let pre_read = request
            .masks
            .pre_read_quote(export::web_snapshot::pre_read_quote(
                source.len(),
                SOURCE_LIMITS,
            )?)?
            .checked_add(request.metadata.retained_charge())
            .ok_or_else(|| resource("Spatial pre-read overflow"))?;
        let profile_budget =
            paintcraft_core::types::resources::SharedBudget::new(WORKER_PAYLOAD_BYTES_MAX);
        let reservation =
            profile_budget.reserve_capacity(pre_read.max(request.spatial.payload_bytes()))?;
        let scope =
            reservation.scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
        let mut baseline = scope.account();
        baseline.resize(pre_read)?;
        let metadata_budget = paintcraft_core::types::resources::SharedBudget::new(
            request.metadata.retained_charge(),
        );
        let mask_budget =
            paintcraft_core::types::resources::SharedBudget::new(request.masks.decode_bytes());
        let doc = pcnd::read_with_admission_checked(
            source,
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
        let plan = SourcePlan::capture_with_policies_checked(
            &doc,
            export::CopyPolicies {
                source_metadata: policy,
                filter_bake: bake,
            },
            check,
        )?;
        if plan.source.state() != request.owner.state
            || plan.size() != request.size
            || plan.source_flags() != request.source_flags
        {
            return Err(failed("Browser MSP2 snapshot disagrees with request"));
        }
        let first_quote = std::cell::Cell::new(0u64);
        if plan.spatial_context(source.len())? != request.spatial.encode() {
            return Err(failed("Spatial export facts disagree with decoded source"));
        }
        baseline.resize(
            request
                .spatial
                .payload_bytes()
                .checked_sub(request.spatial.workspace_bytes())
                .ok_or_else(|| resource("Spatial payload extent"))?,
        )?;
        let baseline = std::cell::RefCell::new(baseline);
        let fallback = paintcraft_render::spatial::WorkspaceAdmission::new(
            scope.clone(),
            request.spatial.workspace_bytes(),
            export::SPATIAL_WORK_MAX,
        );
        let artifact = export::msp::prepare_document_with_policies(
            &doc,
            Version::RowRleV2,
            &LOSSES.map(str::to_owned),
            |layout, resident| {
                let quote = export::msp::render_reservation_bytes(layout)?
                    .checked_add(resident)
                    .and_then(|n| n.checked_add(request.spatial.workspace_bytes()))
                    .and_then(|n| n.checked_add(source.len() as u64))
                    .and_then(|n| n.checked_add(request.metadata.retained_charge()))
                    .and_then(|n| n.checked_add(request.masks.decode_bytes()))
                    .and_then(|n| n.checked_add(filters.filter_storage_bytes()))
                    // Existing parser metadata ceiling, framing/thumbnail and
                    // miscellaneous bounded codec/facade metadata payload.
                    .and_then(|n| {
                        n.checked_add(
                            (8 << 20)
                                + (1 << 20)
                                + ENCODED_BYTES_MAX as u64
                                + 2 * PREVIEW_BYTES_MAX as u64,
                        )
                    })
                    .ok_or_else(|| resource("Browser MSP2 workspace overflow"))?;
                if resident > RESIDENT_BYTES_MAX || quote > WORKER_PAYLOAD_BYTES_MAX {
                    return Err(resource("Browser MSP2 worker payload allowance"));
                }
                first_quote.set(quote);
                check()
            },
            |plan| {
                let encoded_phase = first_quote
                    .get()
                    .checked_sub(request.spatial.workspace_bytes())
                    .and_then(|n| n.checked_add(plan.additional_bytes() as u64))
                    .ok_or_else(|| resource("MSP encode-phase overflow"))?;
                if plan.encoded_bytes() > ENCODED_BYTES_MAX
                    || encoded_phase > WORKER_PAYLOAD_BYTES_MAX
                {
                    return Err(resource("Browser MSP2 encoding allowance"));
                }
                baseline.borrow_mut().resize(encoded_phase)?;
                check()
            },
            |doc, check| {
                paintcraft_render::spatial::with_workspace_or_current(
                    fallback.clone(),
                    request.spatial.workspace_bytes(),
                    export::SPATIAL_WORK_MAX,
                    || paintcraft_render::render_cancellable(doc, check),
                )
            },
            check,
            export::CopyPolicies {
                source_metadata: policy,
                filter_bake: bake,
            },
        )?;
        // Composite is freed. Actual encoded pixels reuse that RGBA allowance;
        // packed decode reuses the already-admitted packed encoding allowance.
        let bitmap = crate::msp::decode(artifact.bytes(), check)?;
        let rgba = bitmap.rgba(check)?;
        let size = preview_size(request.size);
        let mut preview = Vec::new();
        preview
            .try_reserve_exact((size.0 * size.1 * 4) as usize)
            .map_err(|_| resource("Browser MSP2 preview allocation"))?;
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
    })();
    match result {
        Ok(bytes) => Ok(bytes),
        Err(error) => response::refused(&request, &error, version),
    }
}
