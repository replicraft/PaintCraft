use super::*;
pub fn hello(
    control: &str,
    actual_control_quote_bytes: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    let value = parse_control(control, CAPABILITY_BYTES_MAX)?;
    object(
        &value,
        &[
            "protocol",
            "registryContract",
            "pairs",
            "appearance",
            "admission",
            "slice",
        ],
    )?;
    header(&value)?;
    check()?;
    appearance(field(&value, "appearance")?)?;
    let admission = admission(&value)?;
    if actual_control_quote_bytes < CONTROL_BYTES {
        return Err(bound());
    }
    let required = requirements(field(&value, "pairs")?, 257, check)?;
    // Check a conservative complete reply bound BEFORE vector/string assembly.
    // IDs below are trusted registry metadata, nevertheless their bounds are closed.
    let mut extent = 4096usize;
    for (profile, codec) in required.requirements().pairs() {
        let entry = capabilities::resolve_for(RegistryContract::LayerAppearance3, profile, codec)?;
        let (resource, _) = entry.resource_contract();
        let (class, _) = entry.workload_class();
        for (id, max) in [(profile, 96), (codec, 64), (resource, 96), (class, 96)] {
            if id.len() > max
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Scoped registry metadata identifier bound",
                ));
            }
            extent = extent.checked_add(id.len()).ok_or_else(resource_error)?;
        }
        extent = extent.checked_add(256).ok_or_else(resource_error)?;
    }
    if extent > CAPABILITY_BYTES_MAX {
        return Err(resource_error());
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(required.requirements().len())
        .map_err(|_| resource_error())?;
    for (profile, codec) in required.requirements().pairs() {
        check()?;
        let entry = capabilities::resolve_for(RegistryContract::LayerAppearance3, profile, codec)?;
        let (resource, revision) = entry.resource_contract();
        let (class, class_revision) = entry.workload_class();
        entries.push(Value::object([
            ("profile_id", Value::str(profile)),
            ("codec_id", Value::str(codec)),
            ("resource_contract_id", Value::str(resource)),
            (
                "resource_contract_revision",
                Value::Int(i64::from(revision)),
            ),
            ("workload_class_id", Value::str(class)),
            (
                "workload_class_revision",
                Value::Int(i64::from(class_revision)),
            ),
        ]));
    }
    admission.validate_requirements(&required)?;
    let (required_sha, resolution_sha) = required.requirements().digests(check)?;
    let runtime = Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("appearanceRevision", Value::Int(1)),
        ("queryRevision", Value::Int(1)),
        ("coverageAbi", Value::str(CoverageAbi::current()?.id())),
        ("sliceRevision", Value::Int(1)),
        ("nativeEdge", Value::Int(64)),
        (
            "baseSliceWork",
            Value::str(Admission7::BASE_SLICE.to_string()),
        ),
        (
            "noiseSliceWork",
            Value::str(admission.noise_slice().to_string()),
        ),
        (
            "controlQuoteBytes",
            Value::str(actual_control_quote_bytes.to_string()),
        ),
    ]);
    let reply = Value::object([
        ("kind", Value::str("capabilities")),
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("entries", Value::Array(entries)),
        ("requiredSha256", Value::str(hex(&required_sha))),
        ("resolutionSha256", Value::str(hex(&resolution_sha))),
        ("appearance", appearance_registration().value()),
        ("admissionSha256", Value::str(hex(&admission.digest()?))),
        ("runtimeFacts", runtime),
    ]);
    if text(&reply).len() > CAPABILITY_BYTES_MAX {
        return Err(resource_error());
    }
    Ok(reply)
}
fn resource_error() -> Error {
    resource("Scoped capability reply extent")
}
