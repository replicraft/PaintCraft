//! Admitted source hierarchy. Only explicit, neutral passthrough groups construct this type.
use super::*;

pub(super) struct NeutralAutoGroup {
    name: String,
    visible: bool,
}
pub(super) enum Kind {
    Root,
    Group(NeutralAutoGroup),
    Layer(Layer),
}
struct Node {
    parent: Option<usize>,
    kind: Kind,
}
pub(super) struct Tree {
    nodes: Vec<Node>,
    limit: usize,
}
impl Tree {
    pub(super) fn new(limits: &Limits, budget: &Budget) -> Result<Self> {
        let limit = limits
            .layers
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| resource("ORA tree node allowance overflows"))?;
        let limit = limit.min(limits.xml_bytes / 3 + 1);
        Ok(Self {
            nodes: budget.vector(limit)?,
            limit,
        })
    }
    pub(super) fn push(&mut self, parent: Option<usize>, kind: Kind) -> Result<usize> {
        if self.nodes.len() >= self.limit {
            return Err(resource("ORA tree node count exceeds allowance"));
        }
        if parent.is_some_and(|parent| parent >= self.nodes.len()) {
            return Err(malformed("ORA tree parent is not an earlier node"));
        }
        let index = self.nodes.len();
        self.nodes.push(Node { parent, kind });
        Ok(index)
    }
    pub(super) fn flatten(
        self,
        leaves: usize,
        budget: &Budget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Vec<Layer>, usize)> {
        let mut visible: Vec<bool> = budget.vector(self.nodes.len())?;
        let mut layers = budget.vector(leaves)?;
        let mut groups = 0;
        for node in self.nodes {
            check()?;
            let parent_visible = node.parent.is_none_or(|parent| visible[parent]);
            let effective = match node.kind {
                Kind::Root => true,
                Kind::Group(group) => {
                    groups += 1;
                    // Explicit conversion loss covers this name and the removed group controls.
                    drop(group.name);
                    parent_visible && group.visible
                }
                Kind::Layer(mut layer) => {
                    layer.visible &= parent_visible;
                    let effective = layer.visible;
                    layers.push(layer);
                    effective
                }
            };
            visible.push(effective);
        }
        Ok((layers, groups))
    }
}

pub(super) fn group(values: Vec<(String, String)>) -> Result<NeutralAutoGroup> {
    let mut name = String::new();
    let mut visible = true;
    let mut explicit_auto = false;
    for (key, value) in values {
        match key.as_str() {
            "name" => name = value,
            "visibility" => {
                visible = match value.as_str() {
                    "visible" => true,
                    "hidden" => false,
                    _ => return Err(malformed("Invalid ORA group visibility")),
                }
            }
            "opacity" => {
                // Conservative exact lexical full-opacity subset: no f64 rounding to one.
                let exact = value == "1"
                    || value.strip_prefix("1.").is_some_and(|fraction| {
                        !fraction.is_empty() && fraction.bytes().all(|b| b == b'0')
                    });
                if !exact {
                    return Err(unsupported(
                        "ORA group conversion requires exact full opacity",
                    ));
                }
            }
            "isolation" if value == "auto" => explicit_auto = true,
            "composite-op" if value == "svg:src-over" => {}
            _ => {
                return Err(unsupported(
                    "ORA group property/isolation/blend is not neutral passthrough",
                ));
            }
        }
    }
    if !explicit_auto {
        return Err(unsupported(
            "ORA isolated or default-isolated groups require another conversion profile",
        ));
    }
    if name.chars().count() > 1024 {
        return Err(resource("ORA group name exceeds allowance"));
    }
    Ok(NeutralAutoGroup { name, visible })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(body: &str, limits: Limits) -> Result<Stack> {
        let xml = format!(r#"<image version="0.0.6" w="1" h="1"><stack>{body}</stack></image>"#);
        super::super::parse_neutral_groups(
            xml.as_bytes(),
            &limits,
            &Budget::new(limits.metadata_bytes),
            &|| Ok(()),
        )
    }
    #[test]
    fn full_opacity_is_lexically_exact_not_f64_near_one() {
        for value in ["1", "1.0", "1.0000000000000000000000"] {
            let body = format!(
                r#"<stack isolation="auto" opacity="{value}"><layer src="data/a.png" x="7" y="-3"/></stack>"#
            );
            let stack = parse(&body, Limits::default()).unwrap();
            assert_eq!(stack.groups, 1);
            assert_eq!(stack.layers[0].origin, (7, -3));
        }
        assert_eq!("0.9999999999999999999999999".parse::<f64>().unwrap(), 1.0);
        for value in [
            "0.9999999999999999999999999",
            "1.0000000000000000000000001",
            "1e0",
            "01",
            "1.",
            "NaN",
            "0",
            "0.5",
        ] {
            let body = format!(
                r#"<stack isolation="auto" opacity="{value}"><layer src="data/a.png"/></stack>"#
            );
            assert_eq!(
                parse(&body, Limits::default()).err().unwrap().code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
    #[test]
    fn all_nodes_and_actual_nested_depth_are_bounded_before_payloads() {
        let empty = r#"<stack isolation="auto"/>"#;
        let body = format!("{empty}{empty}<layer src=\"data/a.png\"/>");
        let error = parse(
            &body,
            Limits {
                layers: 1,
                ..Limits::default()
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        let nested = r#"<stack isolation="auto"><stack isolation="auto"><layer src="data/a.png"/></stack></stack>"#;
        assert_eq!(
            parse(
                nested,
                Limits {
                    depth: 4,
                    ..Limits::default()
                }
            )
            .err()
            .unwrap()
            .code,
            ErrorCode::ResourceLimit
        );
        assert!(
            parse(
                nested,
                Limits {
                    depth: 5,
                    ..Limits::default()
                }
            )
            .is_ok()
        );
        let large = format!(
            r#"<stack name="{}" isolation="auto"><layer src="data/a.png"/></stack>"#,
            "a".repeat(1025)
        );
        assert_eq!(
            parse(&large, Limits::default()).err().unwrap().code,
            ErrorCode::ResourceLimit
        );
    }
}
