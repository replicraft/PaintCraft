//! Source7 ControlWorkClass revision1, structural slots, never CPU/RSS units.
use super::*;
pub(super) const CHECK_SLOTS: u64 = 512;
pub(super) fn parse_quote(bytes: usize) -> Result<u64> {
    u64::try_from(bytes)
        .ok()
        .and_then(|n| n.checked_mul(64))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| resource("Source7 parse work"))
}
pub(super) fn checked<'a>(
    job: &'a OpenJob,
    check: &'a dyn Fn() -> Result<()>,
) -> impl Fn() -> Result<()> + 'a {
    move || {
        job.spend_control(CHECK_SLOTS)?;
        check()
    }
}
pub(super) fn fact(
    required: &Requirements7,
    admission: Admission7,
    inventory: Inventory7,
    job: &OpenJob,
    check: &dyn Fn() -> Result<()>,
) -> Result<Fact7> {
    job.spend_control(metadata::framing_work_quote(required)?)?;
    Fact7::capture(required, admission, inventory, check)
}
pub(super) fn union(
    required: &Requirements7,
    admission: Admission7,
    source: [u8; 32],
    job: &OpenJob,
    check: &dyn Fn() -> Result<()>,
) -> Result<Union7> {
    job.spend_control(metadata::framing_work_quote(required)?)?;
    Union7::capture(required, admission, source, check)
}
