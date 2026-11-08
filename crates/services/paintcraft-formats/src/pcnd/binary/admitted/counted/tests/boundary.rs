use super::*;
use std::io::Write;
#[test]
fn whole_container_limit_includes_the_thirty_two_byte_trailer() {
    for body_extra in [0, 1] {
        let budget = SharedBudget::new(4096);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut sink = counting::CountingSink::new(&scope).unwrap();
        let body = SOURCE_MAX - 32 + body_extra;
        let chunk = [0; 4096];
        for _ in 0..body / 4096 {
            sink.write_all(&chunk).unwrap();
        }
        sink.write_all(&chunk[..(body % 4096) as usize]).unwrap();
        assert_eq!(sink.bytes(), body);
        let result = sink.write_all(&[0; 32]);
        if body_extra == 0 {
            result.unwrap();
            assert_eq!(sink.bytes(), 67108864);
            assert_eq!(sink.stop(), None);
        } else {
            assert!(result.is_err());
            assert_eq!(sink.bytes(), 67108833);
            assert_eq!(sink.stop(), Some(counting::CountStop::Limit));
            assert!(sink.write(&[]).is_err());
            assert!(sink.flush().is_err());
        }
        drop(sink);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
