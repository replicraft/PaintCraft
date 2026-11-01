//! Main independently binds contract2 facts; no Worker-owned hash is authority.
use super::*;
use paintcraft_formats::export::{
    noise_workload::Workload,
    prepared_capabilities::{RegistryContract, Requirements},
};
use paintcraft_render::noise_work::NoiseJobPolicy;
fn workload() -> Result<Workload> {
    Workload::browser(NoiseJobPolicy::BrowserV1.max_limits())
}
fn js(value: &json::Value) -> Result<JsValue> {
    let bytes = json::canonical(value);
    if bytes.len() > 64 << 10 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Source6 metadata extent",
        ));
    }
    js_sys::JSON::parse(
        std::str::from_utf8(&bytes).map_err(|_| failed("Source6 metadata encoding"))?,
    )
    .map_err(error)
}
fn capability(required: &Requirements, w: Workload) -> Result<[u8; 32]> {
    required.validate_contract(RegistryContract::ScopedNoise2, &|| Ok(()))?;
    let (r, s) = required.digests(&|| Ok(()))?;
    let mut hash = Sha256::new();
    hash.update(b"paintcraft.worker-capabilities/v2\0");
    hash.update(2u16.to_be_bytes());
    hash.update(r);
    hash.update(s);
    hash.update(w.digest()?);
    hash.update(
        u16::try_from(required.len())
            .map_err(|_| failed("Source6 count"))?
            .to_be_bytes(),
    );
    for (profile, codec) in required.pairs() {
        let entry = paintcraft_formats::export::prepared_capabilities::resolve_for(
            RegistryContract::ScopedNoise2,
            profile,
            codec,
        )?;
        let (class, revision) = entry.workload_class();
        for id in [profile, codec, class] {
            hash.update(
                u16::try_from(id.len())
                    .map_err(|_| failed("Source6 ID"))?
                    .to_be_bytes(),
            );
            hash.update(id.as_bytes());
        }
        hash.update(revision.to_be_bytes());
    }
    Ok(hash.finalize().into())
}
fn fact(required: &Requirements, pairs: bool) -> Result<JsValue> {
    let w = workload()?;
    let (r, s) = required.digests(&|| Ok(()))?;
    let value = object(&[
        ("count", (required.len() as u32).into()),
        ("requiredSha256", hex(&r).into()),
        ("resolutionSha256", hex(&s).into()),
        ("capabilitySha256", hex(&capability(required, w)?).into()),
    ])?;
    if pairs {
        let rows = Array::new();
        for (profile, codec) in required.pairs() {
            rows.push(&object(&[
                ("profile_id", profile.into()),
                ("codec_id", codec.into()),
            ])?);
        }
        Reflect::set(&value, &"pairs".into(), &rows).map_err(error)?;
    }
    Ok(value)
}
pub(super) fn open(plan: &JsValue, source: &Requirements) -> Result<()> {
    for (k, v) in [("protocol", 6u32), ("registryContract", 2)] {
        Reflect::set(plan, &k.into(), &v.into()).map_err(error)?;
    }
    Reflect::set(plan, &"workload".into(), &js(&workload()?.value()?)?).map_err(error)?;
    Reflect::set(plan, &"sourceRequirements".into(), &fact(source, true)?).map_err(error)?;
    Ok(())
}
pub(super) fn attach(plan: &JsValue, b: &super::super::requirements::Bindings) -> Result<()> {
    for (k, r) in [
        ("candidateRequirements", &b.candidate),
        ("unionRequirements", &b.union),
    ] {
        Reflect::set(plan, &k.into(), &fact(r, false)?).map_err(error)?;
    }
    Ok(())
}
pub(super) fn validate(reply: &JsValue, b: &super::super::requirements::Bindings) -> Result<()> {
    number(reply, "protocol", 6)?;
    number(reply, "registryContract", 2)?;
    string(reply, "workloadSha256", &hex(&workload()?.digest()?))?;
    for (k, r) in [
        ("sourceRequirements", &b.source),
        ("candidateRequirements", &b.candidate),
        ("unionRequirements", &b.union),
    ] {
        let actual = field(reply, k)?;
        let (required, resolution) = r.digests(&|| Ok(()))?;
        number(&actual, "count", r.len() as u32)?;
        string(&actual, "requiredSha256", &hex(&required))?;
        string(&actual, "resolutionSha256", &hex(&resolution))?;
        string(
            &actual,
            "capabilitySha256",
            &hex(&capability(r, workload()?)?),
        )?;
    }
    Ok(())
}
/// Facts come from this admitted Worker's actual Wasm getters before source transfer.
pub(super) fn validate_runtime(client: &JsValue) -> Result<()> {
    let runtime = field(client, "runtimeFacts")?;
    let q = field(&runtime, "controlQuote")?;
    let read = |key: &str| -> Result<u64> {
        field(&q, key)?
            .as_f64()
            .filter(|n| n.is_finite() && n.fract() == 0.0 && *n > 0.0 && *n <= (64u64 << 20) as f64)
            .map(|n| n as u64)
            .ok_or_else(|| failed("Source6 actual ABI quote"))
    };
    if read("frameBytes")? > read("retainedBytes")? || read("handleBytes")? > 32 << 20 {
        return Err(failed("Source6 ABI shape"));
    }
    number(&q, "extraOutputBytes", 4_194_304 * 4)?;
    let p = field(&runtime, "slicePolicy")?;
    number(&p, "nativeEdge", 64)?;
    number(&p, "baseSliceLimit", 8_388_608)?;
    number(&p, "noiseSliceLimit", 134_217_728)?;
    Ok(())
}
