//! Codec-independent graph framing; these synthetic bytes are not AV1 fixtures.
use super::*;
fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    [&((8 + body.len()) as u32).to_be_bytes()[..], kind, body].concat()
}
fn full_box(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
    boxed(kind, &[&[version, 0, 0, 0][..], body].concat())
}
fn fixture(association: u8, length: u32) -> Vec<u8> {
    let item = full_box(
        b"infe",
        2,
        &[&[0, 1, 0, 0][..], b"av01", b"picture\0"].concat(),
    );
    let inventory = full_box(b"iinf", 0, &[&[0, 1][..], &item].concat());
    let property = boxed(b"av1C", &[0x81, 0, 0, 0]);
    let associations = full_box(
        b"ipma",
        0,
        &[&1u32.to_be_bytes()[..], &[0, 1, 1, association]].concat(),
    );
    let properties = boxed(b"iprp", &[boxed(b"ipco", &property), associations].concat());
    let location = full_box(
        b"iloc",
        1,
        &[
            &[0x44, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1][..],
            &0u32.to_be_bytes(),
            &length.to_be_bytes(),
        ]
        .concat(),
    );
    let primary = full_box(b"pitm", 0, &[0, 1]);
    let meta = full_box(
        b"meta",
        0,
        &[
            primary,
            inventory,
            properties,
            location,
            boxed(b"idat", b"abc"),
        ]
        .concat(),
    );
    [boxed(b"ftyp", b"avif\0\0\0\0avif"), meta].concat()
}
fn inspect(bytes: &[u8], known: bool) -> Result<Inspection<'_, [u8; 4]>> {
    let tree = paintcraft_paint::inspect_with_layout(
        bytes,
        &paintcraft_paint::Limits::default(),
        &|| Ok(()),
        &layout,
    )?;
    let meta = tree
        .boxes
        .iter()
        .position(|b| b.parent.is_none() && b.kind == *b"meta")
        .unwrap();
    let pitm = unique_child(&tree, meta, *b"pitm")?.unwrap();
    let inventory = unique_child(&tree, meta, *b"iinf")?.unwrap();
    let primary_id = metadata::primary(tree.boxes[pitm].payload)?;
    let mut items = metadata::items(&tree, inventory, &|| Ok(()))?;
    let iprp = unique_child(&tree, meta, *b"iprp")?.unwrap();
    properties::associations(
        &tree,
        iprp,
        &mut items,
        &|b| Ok(b.kind),
        &|_| known,
        &|| Ok(()),
    )?;
    let iloc = unique_child(&tree, meta, *b"iloc")?.unwrap();
    let mut result = Inspection {
        primary_id,
        items,
        references: Vec::new(),
    };
    payloads::resolve(bytes, &tree, meta, iloc, &mut result, &|| Ok(()))?;
    Ok(result)
}
#[test]
fn av01_graph_uses_callback_owned_essential_property_and_finite_storage() {
    let bytes = fixture(0x81, 3);
    let graph = inspect(&bytes, true).unwrap();
    let primary = graph.primary().unwrap();
    assert_eq!(primary.kind, *b"av01");
    assert_eq!(primary.name, "picture");
    assert_eq!(primary.properties[0].property, *b"av1C");
    assert!(primary.properties[0].essential);
    assert_eq!(primary.properties[0].index, 1);
    let data = primary.payload.as_ref().unwrap();
    assert_eq!(data.parts, [b"abc".as_slice()]);
    assert_eq!(data.byte_len, 3);
    assert!(data.grid.is_none());
}
#[test]
fn callback_refusal_bad_property_identity_and_outside_storage_fail_closed() {
    assert_eq!(
        inspect(&fixture(0x81, 3), false).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(inspect(&fixture(0x82, 3), true).is_err());
    assert!(inspect(&fixture(0x80, 3), true).is_err());
    assert!(inspect(&fixture(0x81, 4), true).is_err());
    assert!(inspect(&fixture(0x81, 0), true).is_err());
}
