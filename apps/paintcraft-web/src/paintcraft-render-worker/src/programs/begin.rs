use super::*;
fn batch(service: &Service, operation: &Value, target: Uuid) -> Result<(Batch, Requirements)> {
    let name = string(field(operation, "op")?)?;
    if name.starts_with("filter.mask.") {
        return Err(mask_preview_error());
    }
    let mut added = Requirements::default();
    if matches!(name, "filter.add" | "filter.update") {
        let kind = string(field(field(operation, "descriptor")?, "kind")?)?;
        // Implementation/pricing, including unknown/default/disabled, is checked
        // before a typed batch or private candidate can allocate metadata.
        added.insert(kind, "params-v1")?;
    }
    let kind = operation
        .get("descriptor")
        .and_then(|v| v.get("kind"))
        .and_then(Value::as_str);
    let schema = Batch::schema_for_operation_fields([(name, kind)]);
    let batch = Batch::from_generated_value(Value::object([
        ("schema", Value::str(schema)),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(paintcraft_document::POLICY)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(service.base.state_id.to_string())),
                (
                    "publication_epoch",
                    field(field(&service.source.value, "owner")?, "epoch")?.clone(),
                ),
            ]),
        ),
        ("operations", Value::Array(vec![operation.clone()])),
    ]))?;
    let Some(Op::Filter(op)) = batch.operations.first() else {
        return Err(bad("Prepared preview requires one filter operation"));
    };
    stable_target(op, target)?;
    if let FilterOp::Add { descriptor, .. } | FilterOp::Update { descriptor, .. } = op {
        let registration = capabilities::resolve(descriptor.id(), "params-v1")?;
        let (params, len) = descriptor.semantic_parameter_bytes();
        if registration.decode(&params[..len])? != *descriptor {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "Prepared operation decoder mismatch",
            ));
        }
    }
    Ok((batch, added))
}
pub(crate) fn prepare(service: &mut Service, control: &str) -> Result<Value> {
    let mut retired = false;
    let result = inner(service, control, &mut retired);
    if retired && result.is_err() {
        service.retire();
    }
    result
}
fn inner(service: &mut Service, control: &str, retired: &mut bool) -> Result<Value> {
    let value = wire::parse_control(control, wire::BEGIN_BYTES_MAX)?;
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
            "candidateRequirements",
            "unionRequirements",
        ],
    )?;
    for name in ["owner", "draft", "sourceSha256"] {
        if field(&value, name)? != field(&service.source.value, name)? {
            return Err(stale());
        }
    }
    let generation = decimal(field(&value, "generation")?)?;
    if service.generation.is_some_and(|old| generation <= old) {
        return Err(stale());
    }
    let view = View::parse(field(&value, "view")?)?;
    let render = field(&value, "render")?
        .as_bool()
        .ok_or_else(|| bad("Prepared render boolean"))?;
    let expected_candidate = Fact::parse(field(&value, "candidateRequirements")?, 256, false)?;
    let expected_union = Fact::parse(field(&value, "unionRequirements")?, 257, false)?;
    let operation = field(&value, "operation")?;
    if paintcraft_core::types::json::canonical(operation).len() > CONTROL_BYTES_MAX {
        return Err(resource("Prepared operation exceeds4096bytes"));
    }
    service.retire();
    service.generation = Some(generation);
    *retired = true;
    let target = field(&value, "target")?;
    let staged = if *operation == Value::Null {
        if *target != Value::Null {
            return Err(bad("Prepared BASE has no target"));
        }
        None
    } else {
        Some(batch(service, operation, uuid(target)?)?)
    };
    let state = service.programs.as_ref().expect("generic source");
    let introduced = staged.as_ref().map(|(_, r)| r).cloned().unwrap_or_default();
    let union = state
        .required
        .requirements()
        .union(&introduced, &|| Ok(()))?;
    expected_union.matches(&union, &|| Ok(()))?;
    // No Session, pixel allocation, history or publication. The union was
    // resolved above while only bounded control/typed-operation storage existed.
    if let Some((batch, _)) = staged {
        service.candidate = Some(
            paintcraft_editor::execute::execute_with_admission_checked(
                &service.base,
                paintcraft_raster::Selection::Inactive,
                &batch,
                paintcraft_raster::Admission::new(0),
                &|| Ok(()),
            )?
            .doc,
        );
    }
    let state = service.programs.as_ref().expect("generic source");
    let doc = service.candidate.as_ref().unwrap_or(&service.base);
    let candidate = Requirements::capture(doc, &|| Ok(()))?;
    expected_candidate.matches(&candidate, &|| Ok(()))?;
    let actual_union = state
        .required
        .requirements()
        .union(&candidate, &|| Ok(()))?;
    expected_union.matches(&actual_union, &|| Ok(()))?;
    let request = DisplayRequest::new(doc, view.rect, view.level)?;
    let [width, height] = request.size();
    let count = u64::from(width) * u64::from(height);
    if count > OUTPUT_PIXELS_MAX {
        return Err(resource("Prepared output exceeds4MP"));
    }
    let native = crate::generic_frame::GenericFrame::prepare(
        doc,
        request,
        state.profile,
        state.scope.clone(),
        render,
    )?;
    let rect = request.region();
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
        (
            "operationSha256",
            Value::str(hex(&Sha256::digest(
                paintcraft_core::types::json::canonical(operation),
            ))),
        ),
        ("sourceRequirements", state.profile.source.value()),
        ("candidateRequirements", expected_candidate.value()),
        ("unionRequirements", expected_union.value()),
    ]);
    if render {
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count as usize * 4)
            .map_err(|_| resource("Prepared output allocation"))?;
        pixels.resize(count as usize * 4, 0);
        service.frame = Some(Frame {
            binding: binding.clone(),
            request,
            pixels,
            column: 0,
            row: 0,
            spatial: None,
            programs: native,
        });
    }
    Ok(binding)
}
