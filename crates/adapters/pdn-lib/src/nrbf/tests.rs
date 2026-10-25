use super::*;
fn limits() -> GraphLimits {
    GraphLimits {
        bytes: 1 << 20,
        metadata: 1 << 20,
        objects: 32,
        array_items: 128,
        members: 16,
        depth: 16,
    }
}
fn header() -> Vec<u8> {
    let mut bytes = vec![0];
    for value in [1i32, -1, 1, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes
}
fn parse(bytes: &[u8]) -> Result<Graph<'_>> {
    Graph::parse(bytes, limits(), &|| Ok(()))
}
fn array(id: i32, count: i32) -> Vec<u8> {
    let mut b = vec![16];
    b.extend(id.to_le_bytes());
    b.extend(count.to_le_bytes());
    b
}
#[test]
fn consumed_message_boundary_does_not_scan_following_pixel_data() {
    let mut bytes = header();
    bytes.extend(array(1, 1));
    bytes.push(9);
    bytes.extend(1i32.to_le_bytes());
    bytes.push(11);
    let end = bytes.len();
    bytes.extend([255; 24]);
    let graph = parse(&bytes).unwrap();
    assert_eq!(graph.consumed, end);
    assert_eq!(graph.array(Value::Ref(1)).unwrap(), [Value::Ref(1)]);
}
#[test]
fn oversized_arrays_depth_and_work_are_rejected_before_unbounded_growth() {
    let mut bytes = header();
    bytes.extend(array(1, i32::MAX));
    bytes.push(11);
    assert_eq!(parse(&bytes).unwrap_err().code, ErrorCode::ResourceLimit);
    let mut bytes = header();
    for id in 1..20 {
        bytes.extend(array(id, 1));
    }
    bytes.extend([10, 11]);
    assert_eq!(parse(&bytes).unwrap_err().code, ErrorCode::ResourceLimit);
    let mut limits = limits();
    limits.metadata = 1;
    assert_eq!(
        Graph::parse(&header(), limits, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn unresolved_references_duplicate_objects_and_invalid_null_runs_are_refused() {
    let mut bytes = header();
    bytes.extend(array(1, 1));
    bytes.push(9);
    bytes.extend(999i32.to_le_bytes());
    bytes.push(11);
    assert!(parse(&bytes).is_err());
    let mut bytes = header();
    bytes.extend(array(1, 0));
    bytes.extend(array(1, 0));
    bytes.push(11);
    assert!(parse(&bytes).is_err());
    for count in [0, 2, 255] {
        let mut bytes = header();
        bytes.extend(array(1, 1));
        bytes.extend([13, count, 11]);
        assert!(parse(&bytes).is_err());
    }
}
#[test]
fn method_calls_runtime_class_records_and_unknown_libraries_are_never_executed() {
    for record in [2, 3, 21, 22, 255] {
        let mut bytes = header();
        bytes.push(record);
        assert_eq!(
            parse(&bytes).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let mut bytes = header();
    bytes.push(12);
    bytes.extend(2i32.to_le_bytes());
    bytes.push(4);
    bytes.extend(b"evil");
    assert_eq!(
        parse(&bytes).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
