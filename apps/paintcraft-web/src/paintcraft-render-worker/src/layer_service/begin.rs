use super::*;
pub(super) fn batch(
    state: &State,
    operation: &Value,
    target: Uuid,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Batch, Requirements7)> {
    let name = string(field(operation, "op")?)?;
    if name.starts_with("filter.mask.") {
        return Err(crate::mask_preview_error());
    }
    if !matches!(
        name,
        "filter.add"
            | "filter.update"
            | "filter.set_enabled"
            | "filter.reorder"
            | "filter.remove"
            | "filter.duplicate"
    ) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 BEGIN accepts numeric filter edits only",
        ));
    }
    let mut added = Requirements::default();
    if matches!(name, "filter.add" | "filter.update") {
        let kind = string(field(field(operation, "descriptor")?, "kind")?)?;
        capabilities::resolve_for(RegistryContract::LayerAppearance3, kind, "params-v1")?;
        added.insert(kind, "params-v1")?;
    }
    let introduced = Requirements7::new(added, metadata::appearance_registration(), check)?;
    state.profile.admission.validate_requirements(&introduced)?;
    let operations = vec![operation.clone()];
    let batch = Batch::from_generated_value(Value::object([
        (
            "schema",
            Value::str(Batch::schema_for_operations(&operations)),
        ),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(paintcraft_document::POLICY)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(state.base.state_id.to_string())),
                (
                    "publication_epoch",
                    field(field(&state.source.value, "owner")?, "epoch")?.clone(),
                ),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ]))?;
    let Some(Op::Filter(op)) = batch.operations.first() else {
        return Err(bad("Source7 requires one numeric filter operation"));
    };
    crate::stable_target(op, target)?;
    if let FilterOp::Add { descriptor, .. } | FilterOp::Update { descriptor, .. } = op {
        let row = capabilities::resolve_for(
            RegistryContract::LayerAppearance3,
            descriptor.id(),
            "params-v1",
        )?;
        let (params, len) = descriptor.semantic_parameter_bytes();
        if row.decode(&params[..len])? != *descriptor {
            return Err(bad("Source7 descriptor decoder identity"));
        }
    }
    Ok((batch, introduced))
}
pub(super) fn prepare(
    service: &mut LayerAppearanceService,
    control: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    service
        .state
        .job
        .spend_control(control::parse_quote(control.len())?)?;
    let parsed = wire::begin(control, check)?;
    parsed.matches_source(&service.state.source, &service.state.profile)?;
    if service
        .state
        .generation
        .is_some_and(|old| parsed.generation <= old)
    {
        return Err(stale());
    }
    service.ensure_no_escape()?;
    service.retire();
    service.state.generation = Some(parsed.generation);
    let result = inner(service, parsed, check);
    if result.is_err() {
        service.retire();
    }
    result
}
fn inner(
    service: &mut LayerAppearanceService,
    parsed: wire::Begin,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    check()?;
    let value = parsed.value();
    let operation = field(value, "operation")?;
    let target = field(value, "target")?;
    let job = service.state.job.clone();
    let checked = control::checked(&job, check);
    let spend = |n| job.spend_control(n);
    let staged = if *operation == Value::Null {
        if *target != Value::Null {
            return Err(bad("Source7 BASE has no target"));
        }
        None
    } else {
        Some(batch(&service.state, operation, uuid(target)?, &checked)?)
    };
    let introduced = staged
        .as_ref()
        .map(|(_, r)| r.clone())
        .unwrap_or(Requirements7::new(
            Requirements::default(),
            metadata::appearance_registration(),
            &checked,
        )?);
    let union = service
        .state
        .adopted
        .requirements()
        .union(&introduced, &checked)?;
    let actual_union = control::union(
        &union,
        service.state.profile.admission,
        service.state.profile.source.source_coverage,
        &job,
        check,
    )?;
    if actual_union != parsed.union {
        return Err(stale());
    }
    let mut provenance = CandidateProvenance::Unchanged;
    if let Some((batch, _)) = staged {
        let duplicate = match &batch.operations[0] {
            Op::Filter(FilterOp::Duplicate {
                filter: FilterRef::Id(source_filter),
                alias,
                ..
            }) => {
                let layer = uuid(target)?;
                coverage7::preflight_duplicate_counts(
                    &service.state.base,
                    layer,
                    *source_filter,
                    check,
                    &spend,
                )?;
                Some((layer, *source_filter, alias.clone()))
            }
            _ => None,
        };
        let staged = paintcraft_editor::execute::metadata_clone::stage_checked(
            &service.state.base,
            &batch,
            paintcraft_raster::Admission::new(0),
            &service.state.metadata_stage,
            check,
            &spend,
        )?;
        if let Some((layer, source_filter, alias)) = duplicate {
            let (owner, new_filter) = *staged
                .filter_aliases()
                .get(&alias)
                .ok_or_else(|| bad("Source7 duplicate alias missing"))?;
            if owner != layer {
                return Err(stale());
            }
            provenance = CandidateProvenance::Duplicate {
                layer,
                source_filter,
                new_filter,
            };
        }
        service.state.candidate = Some(staged);
    }
    let state = &mut service.state;
    let doc = state
        .candidate
        .as_ref()
        .map(|c| c.document())
        .unwrap_or(&state.base);
    let numerical = Requirements::capture(doc, &checked)?;
    let candidate = Requirements7::new(numerical, metadata::appearance_registration(), &checked)?;
    state.profile.admission.validate_requirements(&candidate)?;
    let inventory = state.adopted.candidate_inventory_checked(
        &state.base,
        doc,
        provenance,
        &state.inventory,
        check,
        &spend,
    )?;
    let fact = control::fact(&candidate, state.profile.admission, inventory, &job, check)?;
    if fact != parsed.candidate {
        return Err(stale());
    }
    let union = state.adopted.requirements().union(&candidate, &checked)?;
    let actual_union = control::union(
        &union,
        state.profile.admission,
        state.profile.source.source_coverage,
        &job,
        check,
    )?;
    if actual_union != parsed.union {
        return Err(stale());
    }
    let request = DisplayRequest::new(doc, parsed.view.rect, parsed.view.level)?;
    let policy = SlicePolicy::for_mode(state.profile.admission.mode == Mode7::Noise);
    let q = frame::AppearanceFrame::quote(doc, request, &job, parsed.render, policy, check)?;
    let [width, height] = request.size();
    let rect = request.region();
    let binding = Value::object([
        (
            "kind",
            Value::str(if parsed.render { "frame" } else { "prepared" }),
        ),
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("generation", Value::str(parsed.generation.to_string())),
        ("owner", field(value, "owner")?.clone()),
        ("draft", field(value, "draft")?.clone()),
        ("sourceSha256", field(value, "sourceSha256")?.clone()),
        ("target", target.clone()),
        ("view", parsed.view.value.clone()),
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
        ("level", Value::Int(i64::from(parsed.view.level))),
        (
            "operationSha256",
            Value::str(hex(&Sha256::digest(
                paintcraft_core::types::json::canonical(operation),
            ))),
        ),
        ("admissionSha256", Value::str(hex(&state.admission_sha256))),
        ("sourceRequirements", state.profile.source.value()),
        ("candidateRequirements", fact.value()),
        ("unionRequirements", actual_union.value()),
        (
            "workQuote",
            quote::value(q.work, q.frame.mask_actual, q.frame.mask_upper)?,
        ),
    ]);
    check()?;
    if parsed.render {
        state.frame = Some(frame::AppearanceFrame::prepare(
            doc,
            request,
            (&job, policy),
            &state.output,
            binding.clone(),
            q,
            check,
        )?);
    }
    state.last_binding = Some(binding.clone());
    Ok(binding)
}
