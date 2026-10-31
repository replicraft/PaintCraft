//! Main's independent requirement bindings for the generic preview protocol.
use super::*;
use paintcraft_formats::export::prepared_capabilities::Requirements;

fn value(requirements: &Requirements, pairs: bool) -> Result<JsValue> {
    let (required, resolution) = requirements.digests(&|| Ok(()))?;
    let value = object(&[
        ("count", (requirements.len() as u32).into()),
        ("requiredSha256", hex(&required).into()),
        ("resolutionSha256", hex(&resolution).into()),
    ])?;
    if pairs {
        let entries = Array::new();
        for (profile, codec) in requirements.pairs() {
            entries.push(&object(&[
                ("profile_id", profile.into()),
                ("codec_id", codec.into()),
            ])?);
        }
        Reflect::set(&value, &"pairs".into(), &entries).map_err(error)?;
    }
    Ok(value)
}

pub(super) fn open(plan: &JsValue, source: &Requirements) -> Result<()> {
    for (key, value) in [
        ("protocol", 5u32),
        ("registryContract", 1),
        ("workspaceBytes", 64 << 20),
        ("workLimit", 268435456),
    ] {
        Reflect::set(plan, &key.into(), &value.into()).map_err(error)?;
    }
    Reflect::set(plan, &"sourceRequirements".into(), &value(source, true)?).map_err(error)?;
    Ok(())
}

pub(super) fn attach(
    plan: &JsValue,
    bindings: &super::super::requirements::Bindings,
) -> Result<()> {
    for (key, requirements) in [
        ("candidateRequirements", &bindings.candidate),
        ("unionRequirements", &bindings.union),
    ] {
        Reflect::set(plan, &key.into(), &value(requirements, false)?).map_err(error)?;
    }
    Ok(())
}

pub(super) fn validate(
    reply: &JsValue,
    bindings: &super::super::requirements::Bindings,
) -> Result<()> {
    for (key, requirements) in [
        ("sourceRequirements", &bindings.source),
        ("candidateRequirements", &bindings.candidate),
        ("unionRequirements", &bindings.union),
    ] {
        let actual = field(reply, key)?;
        let (required, resolution) = requirements.digests(&|| Ok(()))?;
        number(&actual, "count", requirements.len() as u32)?;
        string(&actual, "requiredSha256", &hex(&required))?;
        string(&actual, "resolutionSha256", &hex(&resolution))?;
    }
    Ok(())
}
