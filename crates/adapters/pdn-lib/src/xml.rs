use super::*;
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};

fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\r' | '\n')
        || matches!(c as u32,0x20..=0xd7ff|0xe000..=0xfffd|0x10000..=0x10ffff)
}
fn name_start(c: char) -> bool {
    matches!(c,':'|'_'|'A'..='Z'|'a'..='z')
        || matches!(c as u32,
        0xc0..=0xd6|0xd8..=0xf6|0xf8..=0x2ff|0x370..=0x37d|0x37f..=0x1fff|
        0x200c..=0x200d|0x2070..=0x218f|0x2c00..=0x2fef|0x3001..=0xd7ff|
        0xf900..=0xfdcf|0xfdf0..=0xfffd|0x10000..=0xeffff)
}
fn validate_name(bytes: &[u8]) -> Result<()> {
    if bytes.len() > 256 {
        return Err(resource("PDN XML name exceeds the byte limit"));
    }
    let name = std::str::from_utf8(bytes).map_err(|_| malformed("Invalid PDN XML name"))?;
    let mut chars = name.chars();
    if !chars.next().is_some_and(name_start) || !chars.all(|c| {
        name_start(c)
            || matches!(c,'-'|'.'|'0'..='9'|'\u{b7}'|'\u{300}'..='\u{36f}'|'\u{203f}'..='\u{2040}')
    }) {
        return Err(malformed("Invalid PDN XML name"));
    }
    Ok(())
}
fn integer(text: &str) -> Result<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(malformed("Invalid PDN XML integer"));
    }
    text.parse()
        .map_err(|_| malformed("PDN XML integer overflow"))
}
#[derive(Default)]
struct Root {
    width: Option<u32>,
    height: Option<u32>,
    layers: Option<u32>,
    version: Option<String>,
    unknown: bool,
}
fn attributes(
    start: &BytesStart<'_>,
    is_root: bool,
    decoder: quick_xml::encoding::Decoder,
    root: &mut Root,
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let mut count = 0usize;
    for attribute in start.attributes() {
        check()?;
        count += 1;
        if count > limits.attributes_per_element {
            return Err(resource("PDN XML has too many attributes"));
        }
        let attribute =
            attribute.map_err(|_| malformed("Malformed or duplicate PDN XML attribute"))?;
        validate_name(attribute.key.as_ref())?;
        if attribute.value.len() > limits.attribute_bytes {
            return Err(resource("PDN XML attribute exceeds the byte limit"));
        }
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .map_err(|_| malformed("Invalid PDN XML attribute entity"))?;
        if !value.chars().all(xml_char) || attribute.value.contains(&b'<') {
            return Err(malformed("Invalid PDN XML attribute value"));
        }
        if !is_root {
            continue;
        }
        match attribute.key.as_ref() {
            b"width" => root.width = Some(integer(&value)?),
            b"height" => root.height = Some(integer(&value)?),
            b"layers" => root.layers = Some(integer(&value)?),
            b"savedWithVersion" => {
                if value.is_empty() || value.len() > 128 {
                    return Err(malformed("Invalid PDN saved-with version"));
                }
                // Retain inert text. Its claimed application version does not
                // qualify any object-graph version or writer profile.
                root.version = Some(value.into_owned());
            }
            b"xmlns" => return Err(unsupported("Namespaced PDN XML root is not qualified")),
            _ => root.unknown = true,
        }
    }
    Ok(())
}
pub(super) fn inspect(
    xml: &str,
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<HeaderHints> {
    for (index, c) in xml.chars().enumerate() {
        if index.is_multiple_of(4096) {
            check()?;
        }
        if !xml_char(c) {
            return Err(malformed("Invalid XML 1.0 character in PDN header"));
        }
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_comments = true;
    let mut root = Root::default();
    let mut depth = 0usize;
    let mut elements = 0usize;
    let mut started = false;
    let mut ended = false;
    let mut declaration = false;
    let mut children = false;
    let mut events = 0usize;
    loop {
        check()?;
        let event = reader
            .read_event()
            .map_err(|_| malformed("Malformed PDN XML header"))?;
        events += 1;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                if ended {
                    return Err(malformed("PDN XML contains multiple roots"));
                }
                validate_name(start.name().as_ref())?;
                elements += 1;
                if elements > limits.xml_elements || depth.saturating_add(1) > limits.xml_depth {
                    return Err(resource("PDN XML structural limit exceeded"));
                }
                if depth == 0 {
                    if started || start.name().as_ref() != b"pdnImage" {
                        return Err(unsupported("Unsupported PDN XML root"));
                    }
                    started = true;
                } else {
                    children = true;
                }
                attributes(
                    &start,
                    depth == 0,
                    reader.decoder(),
                    &mut root,
                    limits,
                    check,
                )?;
                if empty {
                    if depth == 0 {
                        ended = true;
                    }
                } else {
                    depth += 1;
                }
            }
            Event::End(end) => {
                validate_name(end.name().as_ref())?;
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| malformed("Unexpected PDN XML end tag"))?;
                if depth == 0 {
                    ended = true;
                }
            }
            Event::Decl(decl) => {
                if started || declaration || events != 1 {
                    return Err(malformed("Misplaced PDN XML declaration"));
                }
                declaration = true;
                let contents = std::str::from_utf8(decl.as_ref())
                    .map_err(|_| malformed("Invalid PDN XML declaration"))?;
                let declaration_attributes = BytesStart::from_content(contents, 3);
                for attribute in declaration_attributes.attributes() {
                    let attribute = attribute
                        .map_err(|_| malformed("Invalid PDN XML declaration attribute"))?;
                    match attribute.key.as_ref() {
                        b"version" | b"encoding" => {}
                        b"standalone" if matches!(attribute.value.as_ref(), b"yes" | b"no") => {}
                        _ => return Err(malformed("Invalid PDN XML declaration attribute")),
                    }
                }

                if decl
                    .version()
                    .map_err(|_| malformed("Invalid PDN XML declaration"))?
                    .as_ref()
                    != b"1.0"
                {
                    return Err(unsupported("Unsupported PDN XML version"));
                }
                if let Some(encoding) = decl.encoding() {
                    let encoding = encoding.map_err(|_| malformed("Invalid PDN XML encoding"))?;
                    if !encoding.eq_ignore_ascii_case(b"utf-8") {
                        return Err(unsupported("PDN XML encoding must be UTF-8"));
                    }
                }
            }
            Event::Text(text) => {
                if depth == 0 && text.as_ref().iter().any(|b| !b.is_ascii_whitespace()) {
                    return Err(malformed("Text outside the PDN XML root"));
                }
                if text.as_ref().windows(3).any(|w| w == b"]]>") {
                    return Err(malformed("Invalid PDN XML text terminator"));
                }
            }
            Event::CData(_) => {
                if depth == 0 {
                    return Err(malformed("CDATA outside the PDN XML root"));
                }
            }
            Event::GeneralRef(reference) => {
                if depth == 0 {
                    return Err(malformed("Entity outside the PDN XML root"));
                }
                match reference
                    .resolve_char_ref()
                    .map_err(|_| malformed("Invalid PDN XML character reference"))?
                {
                    Some(c) if xml_char(c) => {}
                    None if matches!(
                        reference.as_ref(),
                        b"lt" | b"gt" | b"amp" | b"apos" | b"quot"
                    ) => {}
                    _ => return Err(malformed("Unknown or invalid PDN XML entity")),
                }
            }
            Event::DocType(_) => {
                return Err(unsupported(
                    "PDN XML document types and external entities are forbidden",
                ));
            }
            Event::PI(_) => {
                return Err(unsupported("PDN XML processing instructions are forbidden"));
            }
            Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    if !started || !ended || depth != 0 {
        return Err(malformed("Incomplete PDN XML root"));
    }
    let width = root
        .width
        .ok_or_else(|| malformed("PDN XML omitted width"))?;
    let height = root
        .height
        .ok_or_else(|| malformed("PDN XML omitted height"))?;
    let layers = root
        .layers
        .ok_or_else(|| malformed("PDN XML omitted layer count"))?;
    if width == 0 || height == 0 {
        return Err(malformed("PDN XML dimensions must be positive"));
    }
    if width > limits.dimension
        || height > limits.dimension
        || u64::from(width) * u64::from(height) > limits.pixels
        || layers > limits.layers
    {
        return Err(resource(
            "PDN XML geometry or layer count exceeds the limit",
        ));
    }
    Ok(HeaderHints {
        width,
        height,
        layers,
        saved_with_version: root.version,
        contains_child_elements: children,
        contains_unknown_root_attributes: root.unknown,
    })
}
