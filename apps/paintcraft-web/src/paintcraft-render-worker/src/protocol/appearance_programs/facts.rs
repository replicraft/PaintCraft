use super::*;
fn own(value: &Value) -> Result<LayerMasks7> {
    object(
        value,
        &[
            "stored",
            "enabled",
            "bands",
            "runs",
            "decodeUpperBytes",
            "identitySha256",
            "coverageQuoteRevision",
            "coverageAbi",
        ],
    )?;
    if integer(field(value, "coverageQuoteRevision")?)? != 1
        || field(value, "coverageAbi")?.as_str() != Some(CoverageAbi::current()?.id())
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 coverage ABI",
        ));
    }
    let result = LayerMasks7 {
        stored: u32::try_from(integer(field(value, "stored")?)?).map_err(|_| bound())?,
        enabled: u32::try_from(integer(field(value, "enabled")?)?).map_err(|_| bound())?,
        bands: integer(field(value, "bands")?)?,
        runs: integer(field(value, "runs")?)?,
        decode_upper_bytes: decimal(field(value, "decodeUpperBytes")?)?,
        identity: hash(field(value, "identitySha256")?)?,
        abi: CoverageAbi::current()?,
    };
    result.validate()?;
    Ok(result)
}
pub(super) fn fact(value: &Value, full: bool) -> Result<Fact7> {
    let names = [
        "count",
        "requiredSha256",
        "resolutionSha256",
        "layerMasks",
        "sourceCoverageSha256",
        "maskInventorySha256",
        "capabilitySha256",
    ];
    if full {
        object(
            value,
            &[
                "count",
                "requiredSha256",
                "resolutionSha256",
                "layerMasks",
                "sourceCoverageSha256",
                "maskInventorySha256",
                "capabilitySha256",
                "pairs",
            ],
        )?;
    } else {
        object(value, &names)?;
    }
    let count = usize::try_from(integer(field(value, "count")?)?).map_err(|_| bound())?;
    if count > 256 {
        return Err(bound());
    }
    Ok(Fact7 {
        count,
        required: hash(field(value, "requiredSha256")?)?,
        resolution: hash(field(value, "resolutionSha256")?)?,
        own: own(field(value, "layerMasks")?)?,
        source_coverage: hash(field(value, "sourceCoverageSha256")?)?,
        mask_inventory: hash(field(value, "maskInventorySha256")?)?,
        capability: hash(field(value, "capabilitySha256")?)?,
    })
}
pub(super) fn union(value: &Value) -> Result<Union7> {
    object(
        value,
        &[
            "count",
            "requiredSha256",
            "resolutionSha256",
            "sourceCoverageSha256",
            "capabilitySha256",
        ],
    )?;
    let count = usize::try_from(integer(field(value, "count")?)?).map_err(|_| bound())?;
    if count > 257 {
        return Err(bound());
    }
    Ok(Union7 {
        count,
        required: hash(field(value, "requiredSha256")?)?,
        resolution: hash(field(value, "resolutionSha256")?)?,
        source_coverage: hash(field(value, "sourceCoverageSha256")?)?,
        capability: hash(field(value, "capabilitySha256")?)?,
    })
}
