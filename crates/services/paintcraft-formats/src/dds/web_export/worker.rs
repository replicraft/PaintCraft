use super::*;
pub fn execute(bytes: &[u8], version: &str, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    if bytes.len() < REQUEST_HEADER || bytes.len() > REQUEST_HEADER + SOURCE_BYTES_MAX {
        return Err(resource("Browser DDS request allowance"));
    }
    let request = Header::decode(&bytes[..REQUEST_HEADER])?;
    let result = (|| {
        let source = &bytes[REQUEST_HEADER..];
        request.check_source(source)?;
        // The existing parser inventories/checks tiles before inflating any
        // pixel payload. Full padded tiles, not logical edge samples, are priced.
        // Price retained profile/control payload before the reader can allocate.
        request.metadata.check_capture_quote(
            source.len() as u64 + RESIDENT_BYTES_MAX + SOURCE_LIMITS.metadata,
            WORKER_PAYLOAD_BYTES_MAX,
        )?;
        let metadata_budget = paintcraft_core::types::resources::SharedBudget::new(
            request.metadata.retained_charge(),
        );
        let doc = pcnd::read_with_metadata_admission_checked(
            source,
            PIXELS_MAX,
            RESIDENT_BYTES_MAX / 4,
            LAYERS_MAX,
            pcnd::MetadataAdmission {
                bytes: u64::from(request.metadata.wire_bytes()),
                budget: &metadata_budget,
            },
            check,
        )?;
        let policy = request.metadata.validate_document(
            &doc,
            export::metadata_policy::Target::new(
                crate::registry::Family::Dds,
                crate::registry::Operation::ExportFlat,
            ),
        )?;
        let options = decode_settings(request.settings, request.size)?;
        let plan = SourcePlan::capture_with_metadata_policy(&doc, options, policy)?;
        if plan.source.state() != request.owner.state
            || plan.size() != request.size
            || plan.settings != request.settings
            || plan.source_flags() != request.source_flags
        {
            return Err(failed("Browser DDS snapshot disagrees with request"));
        }
        let artifact = export::dds::prepare_document_with_metadata_policy(
            &doc,
            options,
            &LOSSES.map(str::to_owned),
            |plan, resident| {
                let quote = export::dds::preparation_reservation_bytes(plan)?
                    .checked_add(resident)
                    .and_then(|n| n.checked_add(source.len() as u64))
                    .and_then(|n| n.checked_add(request.metadata.retained_charge()))
                    // Existing PCND parser metadata allowance, decoder scratch and
                    // the separate response/thumbnail while encoded bytes survive.
                    .and_then(|n| {
                        n.checked_add(
                            (8 << 20)
                                + 1_048_576
                                + ENCODED_BYTES_MAX as u64
                                + PREVIEW_BYTES_MAX as u64 * 2,
                        )
                    })
                    .ok_or_else(|| resource("Browser DDS workspace overflow"))?;
                if resident > RESIDENT_BYTES_MAX
                    || plan.encoded_bytes() > ENCODED_BYTES_MAX as u64
                    || quote > WORKER_PAYLOAD_BYTES_MAX
                {
                    return Err(resource("Browser DDS worker payload allowance"));
                }
                check()
            },
            |doc, check| paintcraft_render::render_cancellable(doc, check),
            check,
            policy,
        )?;
        // The facade's composite has dropped; this same logical RGBA slot is
        // now available for actual encoded top-level pixels, never a rerender.
        let decoded = decode_top_mip_checked(artifact.encoded().bytes(), PIXELS_MAX, check)?;
        let size = preview_size(request.size);
        let mut preview = Vec::new();
        preview
            .try_reserve_exact((size.0 * size.1 * 4) as usize)
            .map_err(|_| resource("Browser DDS preview allocation"))?;
        for y in 0..size.1 {
            check()?;
            for x in 0..size.0 {
                let sx = u64::from(x) * u64::from(request.size.0) / u64::from(size.0);
                let sy = u64::from(y) * u64::from(request.size.1) / u64::from(size.1);
                let at = (sy * u64::from(request.size.0) + sx) as usize * 4;
                preview.extend_from_slice(&decoded.rgba[at..at + 4]);
            }
        }
        drop(decoded);
        check()?;
        response::success(&request, &artifact, &preview, size, version)
    })();
    match result {
        Ok(bytes) => Ok(bytes),
        Err(error) => response::refused(&request, &error, version),
    }
}
