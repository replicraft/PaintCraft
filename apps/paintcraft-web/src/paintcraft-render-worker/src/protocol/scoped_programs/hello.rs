use super::*;
pub fn hello(control: &str, check: &dyn Fn() -> Result<()>) -> Result<Value> {
    let value = parse_control(control, CAPABILITY_BYTES_MAX)?;
    object(
        &value,
        &["protocol", "registryContract", "pairs", "workload"],
    )?;
    header(&value)?;
    check()?;
    let workload = Workload::parse_browser(field(&value, "workload")?)?;
    let required = requirements_checked(field(&value, "pairs")?, 257, check)?;
    // Check a conservative complete reply bound BEFORE vector/string assembly.
    // IDs below are trusted registry metadata, nevertheless their bounds are closed.
    let mut extent = 1024usize;
    for (profile, codec) in required.pairs() {
        let entry = capabilities::resolve_for(RegistryContract::ScopedNoise2, profile, codec)?;
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
        .try_reserve_exact(required.len())
        .map_err(|_| resource_error())?;
    for (profile, codec) in required.pairs() {
        check()?;
        let entry = capabilities::resolve_for(RegistryContract::ScopedNoise2, profile, codec)?;
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
    let fact = Fact::capture(&required, workload, check)?;
    let reply = Value::object([
        ("kind", Value::str("capabilities")),
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
        ("entries", Value::Array(entries)),
        ("requiredSha256", Value::str(hex(&fact.base.required))),
        ("resolutionSha256", Value::str(hex(&fact.base.resolution))),
        ("workloadSha256", Value::str(hex(&workload.digest()?))),
        ("capabilitySha256", Value::str(hex(&fact.capability))),
    ]);
    if text(&reply).len() > CAPABILITY_BYTES_MAX {
        return Err(resource_error());
    }
    Ok(reply)
}
fn resource_error() -> Error {
    resource("Scoped capability reply extent")
}
