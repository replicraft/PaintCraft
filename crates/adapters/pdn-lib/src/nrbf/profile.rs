//! Declared field types are part of the supported raster profile, not just names.
use super::*;
#[derive(Clone, Copy)]
enum Expected {
    Primitive(u8),
    String,
    Class(&'static str),
    System(&'static str),
    Pairs,
    ObjectArray,
}
impl Expected {
    fn matches(self, binding: Binding<'_>) -> bool {
        match self {
            Self::Primitive(kind) => binding.kind == 0 && binding.primitive == kind,
            Self::String => binding.kind == 1,
            Self::Class(name) => binding.kind == 4 && binding.class == Some(name),
            Self::System(name) => binding.kind == 3 && binding.class == Some(name),
            Self::Pairs => {
                binding.kind == 3
                    && binding.class.and_then(|name| name.strip_suffix("[]")) == Some(PAIRS)
            }
            Self::ObjectArray => binding.kind == 5,
        }
    }
}
impl Graph<'_> {
    pub fn validate_raster_schema(&self, check: &dyn Fn() -> Result<()>) -> Result<()> {
        use Expected::*;
        for schema in &self.schemas {
            check()?;
            let expected: &[(&str, Expected)] = match schema.name {
                "PaintDotNet.Document" => &[
                    ("isDisposed", Primitive(1)),
                    ("layers", Class("PaintDotNet.LayerList")),
                    ("width", Primitive(8)),
                    ("height", Primitive(8)),
                    ("savedWith", System("System.Version")),
                    ("userMetadataItems", Pairs),
                ],
                "PaintDotNet.LayerList" => &[
                    ("parent", Class("PaintDotNet.Document")),
                    ("ArrayList+_items", ObjectArray),
                    ("ArrayList+_size", Primitive(8)),
                    ("ArrayList+_version", Primitive(8)),
                ],
                "System.Version" => &[
                    ("_Major", Primitive(8)),
                    ("_Minor", Primitive(8)),
                    ("_Build", Primitive(8)),
                    ("_Revision", Primitive(8)),
                ],
                "PaintDotNet.BitmapLayer" => &[
                    (
                        "properties",
                        Class("PaintDotNet.BitmapLayer+BitmapLayerProperties"),
                    ),
                    ("surface", Class("PaintDotNet.Surface")),
                    ("Layer+isDisposed", Primitive(1)),
                    ("Layer+width", Primitive(8)),
                    ("Layer+height", Primitive(8)),
                    (
                        "Layer+properties",
                        Class("PaintDotNet.Layer+LayerProperties"),
                    ),
                ],
                "PaintDotNet.BitmapLayer+BitmapLayerProperties" => {
                    &[("blendOp", Class("PaintDotNet.UserBlendOps+NormalBlendOp"))]
                }
                "PaintDotNet.Surface" => &[
                    ("width", Primitive(8)),
                    ("height", Primitive(8)),
                    ("stride", Primitive(8)),
                    ("scan0", Class("PaintDotNet.MemoryBlock")),
                ],
                "PaintDotNet.Layer+LayerProperties" => &[
                    ("name", String),
                    ("userMetadataItems", Pairs),
                    ("visible", Primitive(1)),
                    ("isBackground", Primitive(1)),
                    ("opacity", Primitive(2)),
                    ("blendMode", Class("PaintDotNet.LayerBlendMode")),
                ],
                "PaintDotNet.MemoryBlock" => &[
                    ("length64", Primitive(9)),
                    ("hasParent", Primitive(1)),
                    ("deferred", Primitive(1)),
                ],
                "PaintDotNet.LayerBlendMode" => &[("value__", Primitive(8))],
                "PaintDotNet.UserBlendOps+NormalBlendOp" => &[],
                PAIRS => &[("key", String), ("value", String)],
                _ => return Err(unsupported("Unqualified PDN raster class schema")),
            };
            if expected.len() != schema.members.len() {
                return Err(unsupported("Unqualified PDN raster field count"));
            }
            for member in &schema.members {
                check()?;
                let kind = expected
                    .iter()
                    .find(|(name, _)| *name == member.name)
                    .map(|(_, kind)| *kind)
                    .ok_or_else(|| unsupported("Unqualified PDN raster field name"))?;
                if !kind.matches(member.binding) {
                    return Err(unsupported(
                        "PDN declared field type is outside the raster profile",
                    ));
                }
            }
        }
        Ok(())
    }
}
