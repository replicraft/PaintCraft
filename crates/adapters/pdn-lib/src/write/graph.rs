//! Allowlisted MS-NRBF records for the observed Paint.NET 5.1.12 bitmap schema.
//! Class/assembly identities are literals from the pinned original templates;
//! IDs and records are serialized independently, never patched from a template.
use super::*;
const DATA: i32 = 1001;
const CORE: i32 = 1002;
const DOCUMENT: &str = "PaintDotNet.Document";
const LIST: &str = "PaintDotNet.LayerList";
const BITMAP: &str = "PaintDotNet.BitmapLayer";
const BITMAP_PROPS: &str = "PaintDotNet.BitmapLayer+BitmapLayerProperties";
const SURFACE: &str = "PaintDotNet.Surface";
const PROPS: &str = "PaintDotNet.Layer+LayerProperties";
const MEMORY: &str = "PaintDotNet.MemoryBlock";
const NORMAL: &str = "PaintDotNet.UserBlendOps+NormalBlendOp";
const MODE: &str = "PaintDotNet.LayerBlendMode";
const PAIRS_ARRAY: &str = "System.Collections.Generic.KeyValuePair`2[[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089],[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]][]";
#[derive(Clone, Copy)]
enum Type {
    Primitive(u8),
    String,
    Array,
    System(&'static str),
    Class(&'static str, i32),
}
impl Type {
    fn tag(self) -> u8 {
        match self {
            Self::Primitive(_) => 0,
            Self::String => 1,
            Self::System(_) => 3,
            Self::Class(_, _) => 4,
            Self::Array => 5,
        }
    }
    fn extra(self, out: &mut Output) -> Result<()> {
        match self {
            Self::Primitive(kind) => out.put(&[kind]),
            Self::System(name) => out.text(name),
            Self::Class(name, library) => {
                out.text(name)?;
                out.le(library)
            }
            _ => Ok(()),
        }
    }
}
fn class(
    out: &mut Output,
    id: i32,
    name: &str,
    library: Option<i32>,
    fields: &[(&str, Type)],
) -> Result<()> {
    out.put(&[if library.is_some() { 5 } else { 4 }])?;
    out.le(id)?;
    out.text(name)?;
    out.le(fields.len() as i32)?;
    for (name, _) in fields {
        out.text(name)?;
    }
    for (_, kind) in fields {
        out.put(&[kind.tag()])?;
    }
    for (_, kind) in fields {
        kind.extra(out)?;
    }
    if let Some(library) = library {
        out.le(library)?;
    }
    Ok(())
}
fn reference(out: &mut Output, id: i32) -> Result<()> {
    out.put(&[9])?;
    out.le(id)
}
fn library(out: &mut Output, id: i32, name: &str) -> Result<()> {
    out.put(&[12])?;
    out.le(id)?;
    out.text(name)
}
pub(super) fn write(
    out: &mut Output,
    doc: &RasterDocument<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    use Type::*;
    out.put(&[0])?;
    for value in [1, -1, 1, 0] {
        out.le(value)?;
    }
    library(
        out,
        DATA,
        "PaintDotNet.Data, Version=5.112.9563.32325, Culture=neutral, PublicKeyToken=null",
    )?;
    library(
        out,
        CORE,
        "PaintDotNet.Core, Version=5.112.9563.32325, Culture=neutral, PublicKeyToken=null",
    )?;
    class(
        out,
        1,
        DOCUMENT,
        Some(DATA),
        &[
            ("isDisposed", Primitive(1)),
            ("layers", Class(LIST, DATA)),
            ("width", Primitive(8)),
            ("height", Primitive(8)),
            ("savedWith", System("System.Version")),
            ("userMetadataItems", System(PAIRS_ARRAY)),
        ],
    )?;
    out.put(&[0])?;
    reference(out, 2)?;
    out.le(doc.width as i32)?;
    out.le(doc.height as i32)?;
    reference(out, 3)?;
    out.put(&[10])?;
    class(
        out,
        2,
        LIST,
        Some(DATA),
        &[
            ("parent", Class(DOCUMENT, DATA)),
            ("ArrayList+_items", Array),
            ("ArrayList+_size", Primitive(8)),
            ("ArrayList+_version", Primitive(8)),
        ],
    )?;
    reference(out, 1)?;
    reference(out, 4)?;
    out.le(doc.layers.len() as i32)?;
    out.le(0)?;
    class(
        out,
        3,
        "System.Version",
        None,
        &[
            ("_Major", Primitive(8)),
            ("_Minor", Primitive(8)),
            ("_Build", Primitive(8)),
            ("_Revision", Primitive(8)),
        ],
    )?;
    for value in doc.saved_with {
        out.le(value as i32)?;
    }
    out.put(&[16])?;
    out.le(4)?;
    out.le(doc.layers.len() as i32)?;
    for index in 0..doc.layers.len() {
        reference(out, 10 + index as i32 * 8)?;
    }
    for (index, layer) in doc.layers.iter().enumerate() {
        check()?;
        let id = 10 + index as i32 * 8;
        class(
            out,
            id,
            BITMAP,
            Some(CORE),
            &[
                ("properties", Class(BITMAP_PROPS, CORE)),
                ("surface", Class(SURFACE, CORE)),
                ("Layer+isDisposed", Primitive(1)),
                ("Layer+width", Primitive(8)),
                ("Layer+height", Primitive(8)),
                ("Layer+properties", Class(PROPS, CORE)),
            ],
        )?;
        reference(out, id + 1)?;
        reference(out, id + 2)?;
        out.put(&[0])?;
        out.le(doc.width as i32)?;
        out.le(doc.height as i32)?;
        reference(out, id + 3)?;
        class(
            out,
            id + 1,
            BITMAP_PROPS,
            Some(CORE),
            &[("blendOp", Class(NORMAL, CORE))],
        )?;
        reference(out, id + 4)?;
        class(
            out,
            id + 2,
            SURFACE,
            Some(CORE),
            &[
                ("width", Primitive(8)),
                ("height", Primitive(8)),
                ("stride", Primitive(8)),
                ("scan0", Class(MEMORY, CORE)),
            ],
        )?;
        out.le(doc.width as i32)?;
        out.le(doc.height as i32)?;
        out.le((doc.width * 4) as i32)?;
        reference(out, id + 5)?;
        class(
            out,
            id + 3,
            PROPS,
            Some(CORE),
            &[
                ("name", String),
                ("userMetadataItems", System(PAIRS_ARRAY)),
                ("visible", Primitive(1)),
                ("isBackground", Primitive(1)),
                ("opacity", Primitive(2)),
                ("blendMode", Class(MODE, CORE)),
            ],
        )?;
        reference(out, id + 7)?;
        out.put(&[
            10,
            u8::from(layer.visible),
            u8::from(layer.is_background),
            layer.opacity,
        ])?;
        reference(out, id + 6)?;
        class(out, id + 4, NORMAL, Some(CORE), &[])?;
        class(
            out,
            id + 5,
            MEMORY,
            Some(CORE),
            &[
                ("length64", Primitive(9)),
                ("hasParent", Primitive(1)),
                ("deferred", Primitive(1)),
            ],
        )?;
        out.put(&(layer.rgba.len() as i64).to_le_bytes())?;
        out.put(&[0, 1])?;
        class(out, id + 6, MODE, Some(CORE), &[("value__", Primitive(8))])?;
        out.le(0)?;
        out.put(&[6])?;
        out.le(id + 7)?;
        out.text(layer.name)?;
    }
    out.put(&[11])
}
