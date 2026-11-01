use super::*;
#[derive(Debug)]
pub struct Begin {
    value: Value,
    pub generation: u64,
    pub view: View,
    pub render: bool,
    pub candidate: Fact7,
    pub union: Union7,
    pub admission_sha256: [u8; 32],
}
impl Begin {
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn into_value(self) -> Value {
        self.value
    }
    /// Check immutable owner/source/workload before retiring an accepted frame.
    /// Candidate grammar/union, generation and publication decisions belong runtime.
    pub fn matches_source(&self, source: &Source, profile: &Profile) -> Result<()> {
        for name in ["owner", "draft", "sourceSha256"] {
            if field(&self.value, name)? != field(&source.value, name)? {
                return Err(stale());
            }
        }
        if self.admission_sha256 != profile.admission.digest()? {
            return Err(stale());
        }
        self.candidate
            .validate_binding(profile.admission, appearance_registration())?;
        self.union
            .validate_binding(profile.admission, appearance_registration())?;
        if self.candidate.source_coverage != profile.source.source_coverage
            || self.union.source_coverage != profile.source.source_coverage
            || self.candidate.own != profile.source.own
        {
            return Err(stale());
        }
        Ok(())
    }
}
pub fn begin(control: &str, check: &dyn Fn() -> Result<()>) -> Result<Begin> {
    let value = parse_control(control, BEGIN_BYTES_MAX)?;
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
            "protocol",
            "registryContract",
            "admissionSha256",
        ],
    )?;
    header(&value)?;
    check()?;
    let owner = field(&value, "owner")?;
    object(owner, &["session", "state", "epoch", "frontend"])?;
    uuid(field(owner, "session")?)?;
    uuid(field(owner, "state")?)?;
    decimal(field(owner, "epoch")?)?;
    if integer(field(owner, "frontend")?)? != 3 {
        return Err(bad("Scoped Studio owner"));
    }
    uuid(field(&value, "draft")?)?;
    digest(field(&value, "sourceSha256")?)?;
    let generation = decimal(field(&value, "generation")?)?;
    let view = View::parse(field(&value, "view")?)?;
    let render = field(&value, "render")?
        .as_bool()
        .ok_or_else(|| bad("Scoped render boolean"))?;
    let candidate = facts::fact(field(&value, "candidateRequirements")?, false)?;
    let union = facts::union(field(&value, "unionRequirements")?)?;
    let admission_sha256 = hash(field(&value, "admissionSha256")?)?;
    let operation = field(&value, "operation")?;
    if json::canonical(operation).len() > CONTROL_BYTES_MAX {
        return Err(resource("Scoped operation exceeds4096bytes"));
    }
    let target = field(&value, "target")?;
    if *operation == Value::Null {
        if *target != Value::Null {
            return Err(bad("Scoped BASE has no target"));
        }
    } else {
        operation
            .as_object()
            .ok_or_else(|| bad("Scoped operation object"))?;
        uuid(target)?;
    }
    Ok(Begin {
        value,
        generation,
        view,
        render,
        candidate,
        union,
        admission_sha256,
    })
}
