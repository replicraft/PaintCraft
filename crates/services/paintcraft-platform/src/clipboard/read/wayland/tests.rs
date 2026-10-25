use super::*;
#[test]
fn only_missing_mime_permits_tiff_and_preferred_errors_stop_without_fallback() {
    let mut requests = vec![];
    let (kind, source) = acquire(&|| Ok(()), |kind| {
        requests.push(kind);
        Ok((kind == Encoding::Tiff).then_some(23))
    })
    .unwrap();
    assert_eq!((kind, source), (Encoding::Tiff, 23));
    assert_eq!(requests, [Encoding::Png, Encoding::Tiff]);
    let mut requests = vec![];
    assert!(
        acquire::<()>(&|| Ok(()), |kind| {
            requests.push(kind);
            Err("provider failed".into())
        })
        .is_err()
    );
    assert_eq!(requests, [Encoding::Png]);
    assert!(
        acquire::<()>(&|| Err("cancelled".into()), |_| panic!(
            "cancelled acquisition must not contact provider"
        ))
        .is_err()
    );
    assert!(acquire::<()>(&|| Ok(()), |_| Ok(None)).is_err());
}
