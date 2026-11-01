//! Additive contract2 binding; legacy required/resolution bytes stay unchanged.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    pub base: BaseFact,
    pub capability: [u8; 32],
}
impl Fact {
    pub fn capture(
        required: &Requirements,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        required.validate_contract(RegistryContract::ScopedNoise2, check)?;
        let base = BaseFact::capture(required, check)?;
        let count =
            u16::try_from(required.len()).map_err(|_| resource("Scoped capability count"))?;
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.worker-capabilities/v2\0");
        hash.update(2u16.to_be_bytes());
        hash.update(base.required);
        hash.update(base.resolution);
        hash.update(workload.digest()?);
        hash.update(count.to_be_bytes());
        for (profile, codec) in required.pairs() {
            check()?;
            let entry = capabilities::resolve_for(RegistryContract::ScopedNoise2, profile, codec)?;
            let (class, revision) = entry.workload_class();
            for id in [profile, codec, class] {
                let length =
                    u16::try_from(id.len()).map_err(|_| resource("Scoped capability ID"))?;
                hash.update(length.to_be_bytes());
                hash.update(id.as_bytes());
            }
            hash.update(revision.to_be_bytes());
        }
        Ok(Self {
            base,
            capability: hash.finalize().into(),
        })
    }
    pub fn parse(value: &Value, max: usize, full: bool) -> Result<Self> {
        object(
            value,
            if full {
                &[
                    "count",
                    "pairs",
                    "requiredSha256",
                    "resolutionSha256",
                    "capabilitySha256",
                ]
            } else {
                &[
                    "count",
                    "requiredSha256",
                    "resolutionSha256",
                    "capabilitySha256",
                ]
            },
        )?;
        let count = usize::try_from(integer(field(value, "count")?)?)
            .map_err(|_| resource("Scoped requirement count"))?;
        if count > max || max > 257 {
            return Err(resource("Scoped requirement count"));
        }
        Ok(Self {
            base: BaseFact {
                count,
                required: hash(field(value, "requiredSha256")?)?,
                resolution: hash(field(value, "resolutionSha256")?)?,
            },
            capability: hash(field(value, "capabilitySha256")?)?,
        })
    }
    pub fn value(self) -> Value {
        let Value::Object(mut value) = self.base.value() else {
            unreachable!("base fact object")
        };
        value.insert("capabilitySha256".into(), Value::str(hex(&self.capability)));
        Value::Object(value)
    }
    pub fn matches(
        self,
        required: &Requirements,
        workload: Workload,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if self != Self::capture(required, workload, check)? {
            return Err(stale());
        }
        Ok(())
    }
}
