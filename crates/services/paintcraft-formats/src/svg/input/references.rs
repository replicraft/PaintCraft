use super::*;
use usvg::roxmltree::NodeId;

pub(super) fn css(value: &str) -> Result<()> {
    if value.contains(['\\', '@']) || value.to_ascii_lowercase().contains("icc-color(") {
        return Err(unsupported(
            "SVG CSS escapes, at-rules and external color profiles are not qualified",
        ));
    }
    urls(value)?;
    Ok(())
}
fn urls(value: &str) -> Result<Vec<String>> {
    let mut clean = String::new();
    let mut tail = value;
    while let Some(at) = tail.find("/*") {
        clean.push_str(&tail[..at]);
        let end = tail[at + 2..]
            .find("*/")
            .ok_or_else(|| malformed("unterminated SVG CSS comment"))?;
        tail = &tail[at + 2 + end + 2..];
    }
    clean.push_str(tail);
    let lower = clean.to_ascii_lowercase();
    let mut position = 0;
    let mut targets = Vec::new();
    while let Some(at) = lower[position..].find("url(") {
        let start = position + at + 4;
        let end = lower[start..]
            .find(')')
            .map(|n| start + n)
            .ok_or_else(|| malformed("unterminated SVG URL reference"))?;
        let target = clean[start..end].trim();
        let target = if target.starts_with(['\'', '"']) {
            if target.len() < 2 || target.as_bytes()[0] != target.as_bytes()[target.len() - 1] {
                return Err(malformed("invalid quoted SVG URL reference"));
            }
            &target[1..target.len() - 1]
        } else {
            target
        };
        let id = target
            .strip_prefix('#')
            .filter(|s| !s.is_empty())
            .ok_or_else(|| unsupported("SVG external URL references are disabled"))?;
        targets.push(id.to_owned());
        position = end + 1;
    }
    Ok(targets)
}
pub(super) fn admit(
    xml: &Document<'_>,
    ids: &HashMap<&str, NodeId>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let nodes: Vec<_> = xml.descendants().filter(|n| n.is_element()).collect();
    let indices: HashMap<_, _> = nodes.iter().enumerate().map(|(i, n)| (n.id(), i)).collect();
    let mut graph = vec![Vec::new(); nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        check()?;
        graph[index].extend(
            node.children()
                .filter(|n| n.is_element())
                .map(|n| indices[&n.id()]),
        );
        let mut targets = Vec::new();
        for attr in node.attributes() {
            if attr.name() == "href" {
                if attr.value().starts_with("data:")
                    && matches!(node.tag_name().name(), "image" | "feImage")
                {
                    continue;
                }
                let target = attr
                    .value()
                    .strip_prefix('#')
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| unsupported("SVG external href references are disabled"))?;
                targets.push(target.to_owned());
            } else if matches!(
                attr.name(),
                "style"
                    | "fill"
                    | "stroke"
                    | "filter"
                    | "clip-path"
                    | "mask"
                    | "marker"
                    | "marker-start"
                    | "marker-mid"
                    | "marker-end"
                    | "cursor"
                    | "color-profile"
            ) {
                css(attr.value())?;
                targets.extend(urls(attr.value())?);
            }
        }
        if node.tag_name().name() == "style" {
            targets.extend(urls(node.text().unwrap_or(""))?);
        }
        for target in targets {
            let id = ids
                .get(target.as_str())
                .ok_or_else(|| malformed("SVG reference targets a missing element ID"))?;
            graph[index].push(indices[id]);
        }
    }
    let mut memo = vec![Memo::default(); graph.len()];
    for index in 0..graph.len() {
        cost(index, &graph, &mut memo, 0, check)?;
    }
    Ok(())
}
#[derive(Clone, Default)]
struct Memo {
    mark: u8,
    cost: u32,
    height: usize,
}
fn cost(
    index: usize,
    graph: &[Vec<usize>],
    memo: &mut [Memo],
    depth: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<u32> {
    check()?;
    if depth > DEPTH_MAX {
        return Err(resource("SVG reference nesting exceeds allowance"));
    }
    if memo[index].mark == 1 {
        return Err(malformed("SVG reference cycle is not qualified"));
    }
    if memo[index].mark == 2 {
        if depth + memo[index].height > DEPTH_MAX {
            return Err(resource("SVG cached reference nesting exceeds allowance"));
        }
        return Ok(memo[index].cost);
    }
    memo[index].mark = 1;
    let mut total = 1u32;
    let mut height = 0;
    for target in &graph[index] {
        total = total
            .checked_add(cost(*target, graph, memo, depth + 1, check)?)
            .filter(|n| *n <= NODES_MAX)
            .ok_or_else(|| resource("SVG expanded references exceed node allowance"))?;
        height = height.max(memo[*target].height + 1);
    }
    memo[index] = Memo {
        mark: 2,
        cost: total,
        height,
    };
    Ok(total)
}
