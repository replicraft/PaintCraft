use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
use std::cell::Cell;
mod controls;
fn rows() -> Vec<json::Value> {
    json::parse(include_bytes!(
        "../../../../../tests/normal-rooted-snapshot-fixtures/oracle.json"
    ))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn bytes(row: &json::Value, key: &str) -> Vec<u8> {
    row.get(key)
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn read_fixture(wire: &[u8]) -> Document {
    let budget = SharedBudget::new(16 << 20);
    crate::pcnd::read_with_admission_checked(
        wire,
        1024, // The authored codec-parity fixture is at most 257 x 3 pixels.
        1 << 20,
        16,
        crate::pcnd::ReadAdmission {
            metadata: crate::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: crate::pcnd::CoverageAdmission {
                bytes: 8 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
fn doc(name: &str) -> Document {
    let row = rows()
        .into_iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    read_fixture(&bytes(&row, "input_hex"))
}
struct Fixed {
    bytes: Vec<u8>,
    _account: paintcraft_core::types::resources::BudgetAccount,
}
impl Fixed {
    fn new(scope: &BudgetScope) -> Self {
        let mut account = scope.account();
        account.resize(65536).unwrap();
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(65536).unwrap();
        assert_eq!(bytes.capacity(), 65536);
        Self {
            bytes,
            _account: account,
        }
    }
}
impl Write for Fixed {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(input.len())
            .is_none_or(|n| n > self.bytes.capacity())
        {
            return Err(std::io::ErrorKind::Other.into());
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encoded(doc: &Document, old: bool) -> (Vec<u8>, u64, Vec<u64>) {
    let budget = SharedBudget::new(4 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let trace = RefCell::new(Vec::new());
    let work = Cell::new(0u64);
    let spend = |n: u64| {
        let next = work.get().checked_add(n).unwrap();
        assert!(next <= 268435456);
        work.set(next);
        trace.borrow_mut().push(n);
        Ok(())
    };
    let mut output = Fixed::new(&scope);
    if old {
        super::super::write_portable_snapshot_admitted_checked(
            doc,
            &mut output,
            &scope,
            &|| Ok(()),
            &spend,
        )
        .unwrap();
    } else {
        write_portable_normal_snapshot_rooted_admitted_checked(
            doc,
            &mut output,
            &scope,
            &|| Ok(()),
            &spend,
        )
        .unwrap();
    }
    let bytes = output.bytes.clone();
    drop(output);
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
    (bytes, work.get(), trace.into_inner())
}
#[test]
fn twenty_independent_normal_wire_literals_and_old_portable_bytes_match() {
    for row in rows() {
        let input = bytes(&row, "input_hex");
        let expected = bytes(&row, "expected_hex");
        let source = read_fixture(&input);
        let before = source.clone();
        let (actual, _, _) = encoded(&source, false);
        assert_eq!(
            actual,
            expected,
            "{}",
            row.get("name").unwrap().as_str().unwrap()
        );
        assert_eq!(actual, encoded(&source, true).0);
        assert_eq!(
            actual[4] as i64,
            row.get("version").unwrap().as_i64().unwrap()
        );
        let fresh = read_fixture(&actual);
        assert!(source.same_semantics(&fresh));
        assert!(source.same_semantics(&before));
        assert_eq!(source.state_id, before.state_id);
    }
}
#[test]
fn codec_heavy_cases_match_old_portable_without_claiming_new_codec_oracle() {
    for pattern in 0..5 {
        let (width, height) = if pattern == 4 { (3, 1) } else { (257, 3) };
        let pixels: Vec<u8> = (0..width * height)
            .flat_map(|i| match pattern {
                0 => [
                    i as u8,
                    (i * 17) as u8,
                    91,
                    if i % 2 == 0 { 0 } else { 255 },
                ],
                1 => [(i % 7) as u8, 23, 71, 97],
                2 => [(i / 97) as u8, 42, 61, 255],
                3 => [19, 43, 71, 97],
                _ => {
                    if i < 2 {
                        [19, 43, 71, 97]
                    } else {
                        [23, 47, 79, 101]
                    }
                }
            })
            .collect();
        let source =
            paintcraft_document::import_image(width, height, &pixels, "portable-parity").unwrap();
        let before = source.clone();
        let actual = encoded(&source, false).0;
        assert_eq!(actual, encoded(&source, true).0);
        let fresh = read_fixture(&actual);
        assert!(source.same_semantics(&fresh));
        assert_eq!(source.state_id, before.state_id);
        assert!(source.same_semantics(&before));
    }
}
#[test]
fn sparse_retained_layer_table_has_exact_same_rooted_trace_and_wire() {
    let compact = doc("v6-catalog-sort-order");
    let mut sparse = compact.clone();
    let layer = sparse.layers[&sparse.roots[0]].clone();
    for i in 0..4096 {
        let mut extra = layer.clone();
        extra.id = uuid::Uuid::from_u128(10000 + i);
        sparse.layers.insert(extra.id, extra);
    }
    for i in 0..4096 {
        sparse.layers.remove(&uuid::Uuid::from_u128(10000 + i));
    }
    assert!(sparse.layers.capacity() > compact.layers.capacity());
    assert_eq!(encoded(&compact, false), encoded(&sparse, false));
    assert_eq!(
        portable_normal_snapshot_rooted_scratch_quote(&compact).unwrap(),
        portable_normal_snapshot_rooted_scratch_quote(&sparse).unwrap()
    );
}
#[test]
fn stored_nonnormal_refuses_before_output_even_when_hidden_or_zero_opacity_default() {
    for (mode, hidden, opacity) in [
        (paintcraft_document::LayerBlendMode::Multiply, true, 255),
        (paintcraft_document::LayerBlendMode::Screen, false, 0),
        (paintcraft_document::LayerBlendMode::Multiply, false, 255),
    ] {
        let mut source = doc("v1-default");
        let id = source.roots[0];
        let layer = source.layers.get_mut(&id).unwrap();
        layer.blend_mode = mode;
        layer.visible = !hidden;
        layer.opacity = opacity;
        let before = source.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut output = Vec::new();
        let error = write_portable_normal_snapshot_rooted_admitted_checked(
            &source,
            &mut output,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(output.is_empty());
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
        assert!(source.same_semantics(&before));
        assert_eq!(source.state_id, before.state_id);
    }
}
