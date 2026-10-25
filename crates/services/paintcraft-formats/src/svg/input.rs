use super::*;
use std::{borrow::Cow, collections::HashMap, io::Read};
use usvg::roxmltree::{Document, ParsingOptions};
mod references;
pub(super) struct Inventory<'a> {
    pub images: Vec<&'a str>,
    pub has_text: bool,
}

pub(super) fn source<'a>(bytes: &'a [u8], check: &dyn Fn() -> Result<()>) -> Result<Cow<'a, str>> {
    if bytes.len() > SOURCE_MAX {
        return Err(resource("SVG encoded source exceeds allowance"));
    }
    check()?;
    if bytes.starts_with(&[31, 139]) {
        let mut decoder = flate2::bufread::GzDecoder::new(std::io::Cursor::new(bytes));
        let mut output = Vec::new();
        let mut chunk = [0u8; 65536];
        loop {
            check()?;
            let n = decoder
                .read(&mut chunk)
                .map_err(|_| malformed("invalid SVGZ gzip stream"))?;
            if n == 0 {
                break;
            }
            if output.len() + n > SOURCE_MAX {
                return Err(resource("SVGZ inflated XML exceeds allowance"));
            }
            output
                .try_reserve(n)
                .map_err(|_| resource("cannot allocate SVGZ XML"))?;
            output.extend_from_slice(&chunk[..n]);
        }
        if decoder.into_inner().position() as usize != bytes.len() {
            return Err(malformed("SVGZ has trailing data or concatenated members"));
        }
        Ok(Cow::Owned(
            String::from_utf8(output).map_err(|_| malformed("SVG XML must be UTF-8"))?,
        ))
    } else {
        Ok(Cow::Borrowed(
            std::str::from_utf8(bytes).map_err(|_| malformed("SVG XML must be UTF-8"))?,
        ))
    }
}
pub(super) fn parse<'a>(source: &'a str, check: &dyn Fn() -> Result<()>) -> Result<Document<'a>> {
    check()?;
    let mut declaration = quick_xml::Reader::from_str(source);
    if let Ok(quick_xml::events::Event::Decl(value)) = declaration.read_event()
        && let Some(encoding) = value.encoding()
    {
        let encoding = encoding.map_err(|_| malformed("invalid SVG XML encoding declaration"))?;
        if !encoding.eq_ignore_ascii_case(b"UTF-8") {
            return Err(unsupported("SVG XML encoding declaration must be UTF-8"));
        }
    }
    let xml = Document::parse_with_options(
        source,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: NODES_MAX,
            ..Default::default()
        },
    )
    .map_err(|error| match error {
        usvg::roxmltree::Error::NodesLimitReached => {
            resource("SVG XML node count exceeds allowance")
        }
        _ => malformed(&format!("invalid SVG XML: {error}")),
    })?;
    check()?;
    if xml.root_element().tag_name().name() != "svg"
        || xml.root_element().tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err(unsupported("SVG root must use the SVG XML namespace"));
    }
    Ok(xml)
}
/// usvg accepts SVG2 pixelated/crisp-edges in CSS but ignores their presentation
/// attributes. Its SVG1 optimizeSpeed attribute uses the same nearest sampler.
/// Substitute that private hint while retaining normal CSS priority.
pub(super) fn normalize_hints(source: &str, xml: &Document<'_>) -> Result<Option<String>> {
    let mut ranges = Vec::new();
    for node in xml.descendants().filter(|n| n.is_element()) {
        for attr in node.attributes() {
            if attr.namespace().is_none()
                && attr.name() == "image-rendering"
                && matches!(attr.value(), "pixelated" | "crisp-edges")
            {
                ranges.push(attr.range_value());
            }
        }
    }
    if ranges.is_empty() {
        return Ok(None);
    }
    let mut normalized = source.to_owned();
    for range in ranges.into_iter().rev() {
        normalized.replace_range(range, "optimizeSpeed");
    }
    if normalized.len() > SOURCE_MAX {
        return Err(resource("SVG normalized XML exceeds allowance"));
    }
    Ok(Some(normalized))
}
pub(super) fn inspect<'a>(
    xml: &'a Document<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inventory<'a>> {
    let mut images = Vec::new();
    let mut has_text = false;
    let mut ids = HashMap::new();
    let mut path_bytes = 0usize;
    let mut style_bytes = 0usize;
    let mut text_bytes = 0usize;
    let mut primitives = 0usize;
    for node in xml.descendants() {
        check()?;
        if node.ancestors().take(DEPTH_MAX + 2).count() > DEPTH_MAX + 1 {
            return Err(resource("SVG XML nesting exceeds allowance"));
        }
        if node.is_pi() {
            return Err(unsupported("SVG processing instructions are not qualified"));
        }
        if node.is_text()
            && node
                .ancestors()
                .any(|parent| parent.has_tag_name(("http://www.w3.org/2000/svg", "text")))
        {
            text_bytes += node.text().map_or(0, str::len);
            if text_bytes > 16384 {
                return Err(resource("SVG text content exceeds allowance"));
            }
        }
        if !node.is_element() {
            continue;
        }
        let name = node.tag_name().name();
        if name.starts_with("fe") {
            primitives += 1;
            if primitives > 64 {
                return Err(resource(
                    "SVG source filter primitive count exceeds allowance",
                ));
            }
        }
        let in_metadata = node
            .ancestors()
            .skip(1)
            .any(|parent| parent.has_tag_name(("http://www.w3.org/2000/svg", "metadata")));
        if matches!(
            name,
            "script"
                | "foreignObject"
                | "animate"
                | "animateMotion"
                | "animateTransform"
                | "set"
                | "color-profile"
        ) {
            return Err(unsupported(
                "SVG active, animated or external color-profile content is not qualified",
            ));
        }
        if !in_metadata
            && (node.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
                || !ELEMENTS.contains(&name))
        {
            return Err(unsupported("SVG contains an unqualified rendering element"));
        }
        if node.attributes().len() > 64 {
            return Err(resource("SVG element has too many attributes"));
        }
        for attr in node.attributes() {
            if attr.value().len() > 65536 && !matches!(name, "image" | "feImage") {
                return Err(resource("SVG attribute exceeds allowance"));
            }
            if attr.name().starts_with("on") {
                return Err(unsupported("SVG event handlers are not qualified"));
            }
            if attr.namespace() == Some("http://www.w3.org/XML/1998/namespace")
                && attr.name() == "base"
            {
                return Err(unsupported("SVG base-URL overrides are not qualified"));
            }
            if attr.name() == "id" && ids.insert(attr.value(), node.id()).is_some() {
                return Err(malformed("SVG contains duplicate element IDs"));
            }
            if matches!(attr.name(), "d" | "points") {
                path_bytes += attr.value().len();
            }
            if attr.name() == "style" {
                style_bytes += attr.value().len();
                references::css(attr.value())?;
            }
            if matches!(attr.name(), "numOctaves" | "order") {
                let max = if attr.name() == "order" { 15.0 } else { 8.0 };
                let values = attr
                    .value()
                    .split(|c: char| c.is_ascii_whitespace() || c == ',')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                if values.is_empty()
                    || values.len() > 2
                    || values.iter().any(|v| {
                        v.parse::<f64>().map_or(true, |n| {
                            !n.is_finite() || n < 1.0 || n > max || n.fract() != 0.0
                        })
                    })
                {
                    return Err(resource("SVG filter complexity exceeds allowance"));
                }
            }
        }
        if name == "style" {
            let value = node.text().unwrap_or("");
            style_bytes += value.len();
            references::css(value)?;
        }
        if path_bytes > 1 << 20 || style_bytes > 65536 {
            return Err(resource(
                "SVG path or stylesheet complexity exceeds allowance",
            ));
        }
        if name == "text" && !in_metadata {
            has_text = true;
        }
        if matches!(name, "image" | "feImage") {
            if let Some(value) = node
                .attribute("href")
                .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))
            {
                if name == "image" && value.starts_with('#') {
                    return Err(unsupported(
                        "SVG image fragment references are not qualified; use a local use element instead",
                    ));
                }
                if !value.starts_with('#') {
                    images.push(value);
                }
            }
            if images.len() > 64 {
                return Err(resource("SVG embedded image count exceeds allowance"));
            }
        }
    }
    references::admit(xml, &ids, check)?;
    Ok(Inventory { images, has_text })
}
const ELEMENTS: &[&str] = &[
    "svg",
    "style",
    "g",
    "defs",
    "symbol",
    "use",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "linearGradient",
    "radialGradient",
    "stop",
    "pattern",
    "clipPath",
    "mask",
    "filter",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDistantLight",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feImage",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "fePointLight",
    "feSpecularLighting",
    "feSpotLight",
    "feTile",
    "feTurbulence",
    "image",
    "text",
    "tspan",
    "textPath",
    "title",
    "desc",
    "metadata",
    "a",
    "switch",
    "view",
    "marker",
];
