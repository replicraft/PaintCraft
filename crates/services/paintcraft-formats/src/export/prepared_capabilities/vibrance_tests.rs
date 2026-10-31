//! Proposed registry checks; uses parent module internals, not registered.
use super::*;
#[test]
fn direct_vibrance_is_strict_and_explicitly_priced_not_a_prepared_lut() {
    let registration = resolve("vibrance-rgb-v1", "params-v1").unwrap();
    assert_eq!(registration.execution(), Execution::DirectVibrance);
    assert_eq!(
        registration.resource_contract(),
        ("direct-vibrance-rgba8-work64-v1", 1)
    );
    for n in [-100i16, 0, 100] {
        let descriptor = registration.decode(&n.to_be_bytes()).unwrap();
        assert_eq!(descriptor.id(), "vibrance-rgb-v1");
        assert!(descriptor_requires_generic(descriptor).unwrap());
    }
    for bytes in [
        &[][..],
        &[0][..],
        &[0; 3][..],
        &(-101i16).to_be_bytes()[..],
        &101i16.to_be_bytes()[..],
    ] {
        assert!(registration.decode(bytes).is_err());
    }
    assert_eq!(
        resolve("vibrance-rgb-v1", "params-v2").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
