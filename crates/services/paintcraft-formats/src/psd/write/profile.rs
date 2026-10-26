//! A single Adobe1039 image resource that the selected public model cannot emit.
//! No layer/channel/composite data is serialized or normalized here.
use super::*;
pub(super) const RESOURCE_BYTES: usize = 12 + SRGB_PROFILE.len() + (SRGB_PROFILE.len() % 2);
fn framing(bytes: &[u8], version: Version, check: &dyn Fn() -> Result<()>) -> Result<()> {
    let length_bytes = if version == Version::Psb { 8 } else { 4 };
    let length = bytes
        .get(34..34 + length_bytes)
        .ok_or_else(|| malformed("PSD layer section length missing"))?;
    let length = if length_bytes == 8 {
        u64::from_be_bytes(length.try_into().unwrap())
    } else {
        u64::from(u32::from_be_bytes(length.try_into().unwrap()))
    };
    let end = usize::try_from(length)
        .ok()
        .and_then(|length| (34 + length_bytes).checked_add(length))
        .filter(|end| end.checked_add(2).is_some_and(|end| end <= bytes.len()))
        .ok_or_else(|| malformed("PSD layer section exceeds private output"))?;
    if bytes.get(end..end + 2) != Some(&[0, 1]) {
        return Err(malformed(
            "PSD composite must use admitted PackBits framing",
        ));
    }
    let channels = u16::from_be_bytes(bytes[12..14].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[14..18].try_into().unwrap());
    let rows = usize::from(channels)
        .checked_mul(height as usize)
        .filter(|_| matches!(channels, 3 | 4) && height > 0)
        .ok_or_else(|| malformed("PSD composite row count"))?;
    let table = rows
        .checked_mul(length_bytes / 2)
        .and_then(|size| end.checked_add(2)?.checked_add(size))
        .filter(|table| *table <= bytes.len())
        .ok_or_else(|| malformed("PSD composite PackBits table exceeds output"))?;
    let mut total = table;
    for (index, row) in bytes[end + 2..table]
        .chunks_exact(length_bytes / 2)
        .enumerate()
    {
        if index % 16384 == 0 {
            check()?;
        }
        let length = if length_bytes == 8 {
            u32::from_be_bytes(row.try_into().unwrap()) as usize
        } else {
            usize::from(u16::from_be_bytes(row.try_into().unwrap()))
        };
        total = total
            .checked_add(length)
            .filter(|total| *total <= bytes.len())
            .ok_or_else(|| malformed("PSD composite rows exceed output"))?;
    }
    if total != bytes.len() {
        return Err(malformed("PSD composite suffix or missing row data"));
    }
    Ok(())
}
pub(super) fn insert(
    bytes: &mut Vec<u8>,
    version: Version,
    capacity: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if bytes.len() < 38
        || bytes.get(..4) != Some(b"8BPS")
        || bytes.get(4..6)
            != Some(&(if version == Version::Psb { 2u16 } else { 1u16 }).to_be_bytes())
        || bytes.get(6..12) != Some(&[0; 6])
        || bytes.get(22..26) != Some(&[0, 8, 0, 3])
        || bytes.get(26..34) != Some(&[0; 8])
    {
        return Err(malformed(
            "PSD ICC adapter requires qualified RGB8 with empty color/resources sections",
        ));
    }
    framing(bytes, version, check)?;
    let end = bytes
        .len()
        .checked_add(RESOURCE_BYTES)
        .filter(|size| *size <= capacity)
        .ok_or_else(|| resource("PSD ICC insertion exceeds declaration"))?;
    if bytes.capacity() < end {
        return Err(resource("PSD ICC insertion lacks preallocated capacity"));
    }
    check()?;
    let previous = bytes.len();
    bytes.resize(end, 0);
    let mut end = previous;
    while end > 34 {
        check()?;
        let start = end.saturating_sub(65536).max(34);
        bytes.copy_within(start..end, start + RESOURCE_BYTES);
        end = start;
    }
    bytes[30..34].copy_from_slice(&(RESOURCE_BYTES as u32).to_be_bytes());
    let resource = &mut bytes[34..34 + RESOURCE_BYTES];
    resource[..4].copy_from_slice(b"8BIM");
    resource[4..6].copy_from_slice(&1039u16.to_be_bytes());
    resource[6..8].fill(0);
    resource[8..12].copy_from_slice(&(SRGB_PROFILE.len() as u32).to_be_bytes());
    resource[12..12 + SRGB_PROFILE.len()].copy_from_slice(SRGB_PROFILE);
    check()
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    fn selected_view(version: Version) -> (Vec<u8>, usize) {
        let doc =
            paintcraft_document::import_image_checked(1, 1, &[11, 29, 47, 128], "ICCtest", &|| {
                Ok(())
            })
            .unwrap();
        let plan = preflight(&doc, version, &WriteLimits::default(), &|| Ok(())).unwrap();
        let mut bytes =
            encode_document(&doc, version, &WriteLimits::default(), &[], &|| Ok(())).unwrap();
        bytes.drain(34..34 + RESOURCE_BYTES);
        bytes[30..34].fill(0);
        (bytes, plan.output_capacity)
    }
    #[test]
    fn exact_profile_and_unexpected_sections_are_checked_before_resource_insertion() {
        assert_eq!(SRGB_PROFILE.len(), 3024);
        assert_eq!(
            paintcraft_core::types::policy::hex(&sha2::Sha256::digest(SRGB_PROFILE)),
            "384b832de3412066743b52a75ee906b6fb9fb8d9e09e936fc2c43223815c6e0a"
        );
        for version in [Version::Psd, Version::Psb] {
            let (original, capacity) = selected_view(version);
            for change in 0..5 {
                let mut bytes = original.clone();
                bytes.try_reserve_exact(capacity).unwrap();
                match change {
                    0 => bytes[30] = 1,
                    1 => bytes[25] = 4,
                    2 => bytes[34..38].fill(255),
                    3 => bytes.push(0),
                    _ => bytes[6] = 1,
                }
                let before = bytes.clone();
                assert!(insert(&mut bytes, version, capacity, &|| Ok(())).is_err());
                assert_eq!(bytes, before);
            }
            let (mut bytes, capacity) = selected_view(version);
            let previous = bytes.clone();
            assert!(
                insert(&mut bytes, version, capacity, &|| Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel"
                )))
                .is_err()
            );
            assert_eq!(bytes, previous);
            insert(&mut bytes, version, capacity, &|| Ok(())).unwrap();
            assert_eq!(&bytes[46..46 + SRGB_PROFILE.len()], SRGB_PROFILE);
        }
    }
}
