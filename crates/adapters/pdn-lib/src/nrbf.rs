//! Inert, allowlisted NRBF graph records. No object construction or assembly loading.
use super::*;
mod parser;
mod profile;
use parser::Parser;

pub(super) const PAIRS: &str = "System.Collections.Generic.KeyValuePair`2[[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089],[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";
#[derive(Clone, Copy, Debug)]
pub(super) struct GraphLimits {
    pub bytes: usize,
    pub metadata: usize,
    pub objects: usize,
    pub array_items: usize,
    pub members: usize,
    pub depth: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Value {
    Null,
    Ref(i32),
    Int(i64),
    UInt(u64),
    Bool(bool),
    Bits(u64),
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Binding<'a> {
    pub kind: u8,
    pub primitive: u8,
    pub class: Option<&'a str>,
    pub library: Option<i32>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Member<'a> {
    pub name: &'a str,
    pub binding: Binding<'a>,
}
#[derive(Debug)]
pub(super) struct Schema<'a> {
    pub id: i32,
    pub name: &'a str,
    pub library: Option<i32>,
    pub members: Vec<Member<'a>>,
}
#[derive(Debug)]
pub(super) enum NodeKind<'a> {
    Object {
        schema: usize,
        values: Vec<Value>,
    },
    Array {
        binding: Binding<'a>,
        values: Vec<Value>,
    },
    String(&'a str),
}
#[derive(Debug)]
pub(super) struct Node<'a> {
    pub id: i32,
    pub offset: usize,
    pub kind: NodeKind<'a>,
}
#[derive(Debug)]
pub(super) struct Graph<'a> {
    pub root: i32,
    pub consumed: usize,
    pub nodes: Vec<Node<'a>>,
    pub schemas: Vec<Schema<'a>>,
}
fn allowed(name: &str) -> bool {
    [
        "PaintDotNet.Document",
        "PaintDotNet.LayerList",
        "System.Version",
        "PaintDotNet.BitmapLayer",
        "PaintDotNet.BitmapLayer+BitmapLayerProperties",
        "PaintDotNet.Layer+LayerProperties",
        "PaintDotNet.Surface",
        "PaintDotNet.MemoryBlock",
        "PaintDotNet.LayerBlendMode",
        "PaintDotNet.UserBlendOps+NormalBlendOp",
        PAIRS,
    ]
    .contains(&name)
}
impl<'a> Graph<'a> {
    pub fn parse(
        bytes: &'a [u8],
        limits: GraphLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Parser::new(bytes, limits, check).parse()
    }
    pub fn node(&self, id: i32) -> Result<&Node<'a>> {
        self.nodes
            .binary_search_by_key(&id, |node| node.id)
            .ok()
            .map(|index| &self.nodes[index])
            .ok_or_else(|| malformed("Unresolved PDN object reference"))
    }
    pub fn object(&self, value: Value, name: &str) -> Result<&Node<'a>> {
        let Value::Ref(id) = value else {
            return Err(malformed("Expected PDN object reference"));
        };
        let node = self.node(id)?;
        match &node.kind {
            NodeKind::Object { schema, .. } if self.schemas[*schema].name == name => Ok(node),
            _ => Err(unsupported("Unexpected PDN object class")),
        }
    }
    pub fn class(&self, node: &Node<'a>) -> Option<&str> {
        if let NodeKind::Object { schema, .. } = &node.kind {
            Some(self.schemas[*schema].name)
        } else {
            None
        }
    }
    pub fn field(&self, node: &Node<'a>, name: &str) -> Result<Value> {
        let NodeKind::Object { schema, values } = &node.kind else {
            return Err(malformed("Expected PDN class fields"));
        };
        self.schemas[*schema]
            .members
            .iter()
            .position(|field| field.name == name)
            .map(|index| values[index])
            .ok_or_else(|| malformed("Required PDN field is absent"))
    }
    pub fn exact_fields(&self, node: &Node<'a>, names: &[&str]) -> Result<()> {
        let NodeKind::Object { schema, .. } = &node.kind else {
            return Err(malformed("Expected PDN class fields"));
        };
        let members = &self.schemas[*schema].members;
        if members.len() != names.len()
            || members.iter().any(|member| !names.contains(&member.name))
        {
            return Err(unsupported("Unqualified PDN class field layout"));
        }
        Ok(())
    }
    pub fn array(&self, value: Value) -> Result<&[Value]> {
        let Value::Ref(id) = value else {
            return Err(malformed("Expected PDN array reference"));
        };
        match &self.node(id)?.kind {
            NodeKind::Array { values, .. } => Ok(values),
            _ => Err(malformed("PDN field is not an array")),
        }
    }
    pub fn text(&self, value: Value) -> Result<&'a str> {
        let Value::Ref(id) = value else {
            return Err(malformed("Expected PDN string reference"));
        };
        match self.node(id)?.kind {
            NodeKind::String(text) => Ok(text),
            _ => Err(malformed("PDN field is not a string")),
        }
    }
}
pub(super) fn integer(value: Value) -> Result<i64> {
    match value {
        Value::Int(n) => Ok(n),
        Value::UInt(n) => {
            i64::try_from(n).map_err(|_| malformed("PDN integer exceeds signed bounds"))
        }
        _ => Err(malformed("PDN field is not an integer")),
    }
}
pub(super) fn boolean(value: Value) -> Result<bool> {
    if let Value::Bool(value) = value {
        Ok(value)
    } else {
        Err(malformed("PDN field is not boolean"))
    }
}

#[cfg(test)]
mod tests;
