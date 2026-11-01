use super::*;
fn batch(
    state: &State,
    operation: &Value,
    target: Uuid,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Batch, Requirements)> {
    let name = string(field(operation, "op")?)?;
    if name.starts_with("filter.mask.") {
        return Err(crate::mask_preview_error());
    }
    let mut added = Requirements::default();
    if matches!(name, "filter.add" | "filter.update") {
        let kind = string(field(field(operation, "descriptor")?, "kind")?)?;
        capabilities::resolve_for(RegistryContract::ScopedNoise2, kind, "params-v1")?;
        added.insert(kind, "params-v1")?;
    }
    added.validate_contract(RegistryContract::ScopedNoise2, check)?;
    let kind = operation
        .get("descriptor")
        .and_then(|d| d.get("kind"))
        .and_then(Value::as_str);
    let schema = Batch::schema_for_operation_fields([(name, kind)]);
    let batch = Batch::from_generated_value(Value::object([
        ("schema", Value::str(schema)),
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
        ("operations", Value::Array(vec![operation.clone()])),
    ]))?;
    let Some(Op::Filter(op)) = batch.operations.first() else {
        return Err(bad("Source6 requires one filter operation"));
    };
    crate::stable_target(op, target)?;
    if let FilterOp::Add { descriptor, .. } | FilterOp::Update { descriptor, .. } = op {
        let registration = capabilities::resolve_for(
            RegistryContract::ScopedNoise2,
            descriptor.id(),
            "params-v1",
        )?;
        let (params, len) = descriptor.semantic_parameter_bytes();
        if registration.decode(&params[..len])? != *descriptor {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "Source6 decoder identity",
            ));
        }
    }
    Ok((batch, added))
}
pub(super) fn prepare(
    service: &mut NoiseService,
    control: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    let parsed = wire::begin(control, check)?;
    parsed.matches_source(&service.state.source, service.state.profile)?;
    if service
        .state
        .generation
        .is_some_and(|old| parsed.generation <= old)
    {
        return Err(stale());
    }
    service.ensure_no_escape()?;
    // Only a valid matching new generation retires prior accepted work. All
    // subsequent failures retire private state and cannot expose a stale frame.
    service.retire();
    service.state.generation = Some(parsed.generation);
    let result = inner(service, parsed, check);
    if result.is_err() {
        service.retire();
    }
    result
}
fn inner(
    service: &mut NoiseService,
    parsed: wire::Begin,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    check()?;
    let value = parsed.value();
    let operation = field(value, "operation")?;
    let target = field(value, "target")?;
    let staged = if *operation == Value::Null {
        if *target != Value::Null {
            return Err(bad("Source6 base has no target"));
        }
        None
    } else {
        Some(batch(&service.state, operation, uuid(target)?, check)?)
    };
    let introduced = staged.as_ref().map(|(_, r)| r).cloned().unwrap_or_default();
    let union = service
        .state
        .required
        .requirements()
        .union(&introduced, check)?;
    union.validate_contract(RegistryContract::ScopedNoise2, check)?;
    parsed
        .union
        .matches(&union, service.state.profile.workload, check)?;
    if let Some((batch, _)) = staged {
        service.state.candidate = Some(
            paintcraft_editor::execute::execute_with_admission_checked(
                &service.state.base,
                paintcraft_raster::Selection::Inactive,
                &batch,
                paintcraft_raster::Admission::new(0),
                check,
            )?
            .doc,
        );
    }
    let state = &mut service.state;
    let doc = state.candidate.as_ref().unwrap_or(&state.base);
    let candidate = Requirements::capture(doc, check)?;
    candidate.validate_contract(RegistryContract::ScopedNoise2, check)?;
    parsed
        .candidate
        .matches(&candidate, state.profile.workload, check)?;
    let union = state.required.requirements().union(&candidate, check)?;
    union.validate_contract(RegistryContract::ScopedNoise2, check)?;
    parsed
        .union
        .matches(&union, state.profile.workload, check)?;
    let request = DisplayRequest::new(doc, parsed.view.rect, parsed.view.level)?;
    let [width, height] = request.size();
    let count = u64::from(width) * u64::from(height);
    if count > OUTPUT_PIXELS_MAX {
        return Err(resource("Source6 output exceeds4MP"));
    }
    let q = frame::NoiseFrame::quote(doc, request, state.profile, state.policy, check)?;
    let rect = request.region();
    let binding = Value::object([
        (
            "kind",
            Value::str(if parsed.render { "frame" } else { "prepared" }),
        ),
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
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
        ("sourceRequirements", state.profile.source.value()),
        ("candidateRequirements", parsed.candidate.value()),
        ("unionRequirements", parsed.union.value()),
        (
            "workloadSha256",
            Value::str(hex(&state.profile.workload.digest()?)),
        ),
        ("workQuote", state.profile.workload.quote_value(q.work)?),
    ]);
    check()?;
    if parsed.render {
        state.frame = Some(frame::NoiseFrame::prepare(
            doc,
            request,
            (state.profile, state.policy),
            (state.workspace.clone(), &state.output),
            binding.clone(),
            q,
            check,
        )?);
    }
    state.last_binding = Some(binding.clone());
    Ok(binding)
}
