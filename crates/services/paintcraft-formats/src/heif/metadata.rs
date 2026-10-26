use super::*;
pub(super) use crate::item_graph::metadata::{Reader, items, primary, references};
pub(super) fn validate_links(value: &Inspection<'_>, check: &dyn Fn() -> Result<()>) -> Result<()> {
    for link in &value.references {
        check()?;
        if value.item(link.from).is_none() || link.to.iter().any(|id| value.item(*id).is_none()) {
            return Err(invalid("HEIF reference names an absent item"));
        }
    }
    for item in &value.items {
        check()?;
        if item.kind == *b"grid" {
            let tiles = value
                .references(*b"dimg", item.id)
                .ok_or_else(|| invalid("HEIF grid lacks ordered tile references"))?;
            if tiles.is_empty()
                || tiles
                    .iter()
                    .any(|id| value.item(*id).is_none_or(|tile| tile.kind != *b"hvc1"))
            {
                return Err(unsupported("HEIF grid tiles must be direct HEVC images"));
            }
        }
        if item.kind == *b"iden" {
            let sources = value
                .references(*b"dimg", item.id)
                .ok_or_else(|| invalid("HEIF identity lacks source"))?;
            if sources.len() != 1 {
                return Err(invalid("HEIF identity requires exactly one source"));
            }
        }
        // Follow only image derivation references, and bound every path before
        // a candidate decoder could recursively visit an item graph.
        let mut stack = Vec::new();
        push(&mut stack, (item.id, 0usize), LINKS_MAX)?;
        let mut visited = 0usize;
        while let Some((id, depth)) = stack.pop() {
            check()?;
            visited += 1;
            if depth > 16 || visited > LINKS_MAX {
                return Err(resource("HEIF derivation exceeds traversal allowance"));
            }
            if let Some(targets) = value.references(*b"dimg", id) {
                for target in targets {
                    if *target == item.id {
                        return Err(invalid("cyclic HEIF derivation"));
                    }
                    push(&mut stack, (*target, depth + 1), LINKS_MAX)?;
                }
            }
        }
    }
    let alpha_count = value
        .items
        .iter()
        .filter(|item| {
            value
                .references(*b"auxl", item.id)
                .is_some_and(|ids| ids.iter().any(|id| value.primary_source(*id)))
                && item.properties.iter().any(|p| {
                    matches!(
                        p.property,
                        Property::Auxiliary {
                            kind: "urn:mpeg:hevc:2015:auxid:1",
                            ..
                        }
                    )
                })
        })
        .count();
    if alpha_count > 1 {
        return Err(unsupported("multiple HEIF alpha auxiliaries are ambiguous"));
    }
    for link in value.references.iter().filter(|r| r.kind == *b"prem") {
        if !value.primary_source(link.from)
            || link.to.len() != 1
            || value.alpha().is_none_or(|alpha| link.to[0] != alpha.id)
        {
            return Err(invalid(
                "HEIF premultiplication must name the primary alpha item",
            ));
        }
    }
    Ok(())
}
