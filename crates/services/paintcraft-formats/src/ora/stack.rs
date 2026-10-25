//! Strict baseline XML; no execution, DTD, external entities or extensions.
use super::*;
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
pub(super) struct Layer {
    pub src: String,
    pub name: String,
    pub origin: (i32, i32),
    pub visible: bool,
    pub opacity: u8,
}
pub(super) struct Stack {
    pub width: u32,
    pub height: u32,
    pub title: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub layers: Vec<Layer>,
    pub losses: Vec<String>,
}
pub(super) fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\r' | '\n')
        || matches!(c as u32,0x20..=0xd7ff|0xe000..=0xfffd|0x10000..=0x10ffff)
}
fn attrs(
    start: &BytesStart<'_>,
    reader: &Reader<&[u8]>,
    limits: &Limits,
    budget: &Budget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<(String, String)>> {
    let mut values = budget.vector(limits.attributes.min(32))?;
    for attr in start.attributes() {
        check()?;
        if values.len() >= limits.attributes.min(32) {
            return Err(resource("ORA XML attribute count exceeds allowance"));
        }
        let attr = attr.map_err(|_| malformed("Malformed or duplicate ORA XML attribute"))?;
        if attr.key.as_ref().len() > 128 || attr.value.len() > limits.attribute_bytes {
            return Err(resource("ORA XML attribute exceeds allowance"));
        }
        let name = std::str::from_utf8(attr.key.as_ref())
            .map_err(|_| malformed("Invalid ORA XML attribute name"))?;
        if !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(unsupported("Namespaced or invalid ORA attribute"));
        }
        let text = attr
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|_| malformed("Invalid ORA XML attribute/entity"))?;
        if attr.value.contains(&b'<') || !text.chars().all(xml_char) {
            return Err(malformed("Invalid ORA XML character"));
        }
        values.push((budget.text(name)?, budget.text(&text)?));
    }
    Ok(values)
}
fn number(text: &str) -> Result<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(malformed("Invalid ORA positive integer"));
    }
    text.parse().map_err(|_| malformed("ORA integer overflow"))
}
fn layer(values: Vec<(String, String)>, losses: &mut Vec<String>) -> Result<Layer> {
    let mut layer = Layer {
        src: String::new(),
        name: String::new(),
        origin: (0, 0),
        visible: true,
        opacity: 255,
    };
    for (key, value) in values {
        match key.as_str() {
            "src" => layer.src = value,
            "name" => layer.name = value,
            "x" => {
                layer.origin.0 = value
                    .parse()
                    .map_err(|_| malformed("ORA x origin exceeds signed 32 bits"))?
            }
            "y" => {
                layer.origin.1 = value
                    .parse()
                    .map_err(|_| malformed("ORA y origin exceeds signed 32 bits"))?
            }
            "visibility" => {
                layer.visible = match value.as_str() {
                    "visible" => true,
                    "hidden" => false,
                    _ => return Err(malformed("Invalid ORA visibility")),
                }
            }
            "opacity" => {
                let opacity: f64 = value
                    .parse()
                    .map_err(|_| malformed("Invalid ORA opacity"))?;
                if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                    return Err(malformed("ORA opacity outside zero to one"));
                }
                layer.opacity = (opacity * 255.0).round() as u8;
                if opacity != f64::from(layer.opacity) / 255.0 {
                    loss(losses, "ora-opacity-quantized");
                }
            }
            "composite-op" if value == "svg:src-over" => {}
            _ => {
                return Err(unsupported(
                    "ORA layer property/blend is outside the flat Normal subset",
                ));
            }
        }
    }
    if layer.src.is_empty() {
        return Err(malformed("ORA layer lacks its PNG source"));
    }
    if layer.name.chars().count() > 1024 {
        return Err(resource("ORA layer name exceeds document allowance"));
    }
    Ok(layer)
}
pub(super) fn parse(
    bytes: &[u8],
    limits: &Limits,
    budget: &Budget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Stack> {
    if bytes.len() > limits.xml_bytes {
        return Err(resource("ORA XML exceeds byte allowance"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| malformed("ORA XML must be UTF-8"))?;
    for (i, c) in text.chars().enumerate() {
        if i.is_multiple_of(4096) {
            check()?;
        }
        if !xml_char(c) {
            return Err(malformed("Invalid ORA XML character"));
        }
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_comments = true;
    let mut state = 0;
    let mut declaration = false;
    let mut width = None;
    let mut height = None;
    let mut title = None;
    let mut xres = None;
    let mut yres = None;
    let mut layers = budget.vector(limits.layers.min(limits.xml_bytes / 8))?;
    let mut losses = Vec::new();
    let mut events = 0usize;
    loop {
        check()?;
        events += 1;
        let event = reader
            .read_event()
            .map_err(|_| malformed("Malformed ORA XML"))?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let depth = match state {
                    0 => 1,
                    1 => 2,
                    2 => 3,
                    _ => 4,
                };
                if depth > limits.depth.min(64) {
                    return Err(resource("ORA XML depth allowance exceeded"));
                }
                let values = attrs(&start, &reader, limits, budget, check)?;
                match (state, start.name().as_ref()) {
                    (0, b"image") => {
                        let mut version = false;
                        for (key, value) in values {
                            match key.as_str() {
                                "w" => width = Some(number(&value)?),
                                "h" => height = Some(number(&value)?),
                                "name" => title = Some(value),
                                "xres" => xres = Some(number(&value)?),
                                "yres" => yres = Some(number(&value)?),
                                "version"
                                    if ["0.0.1", "0.0.2", "0.0.3", "0.0.4", "0.0.5", "0.0.6"]
                                        .contains(&value.as_str()) =>
                                {
                                    version = true
                                }
                                _ => {
                                    return Err(unsupported(
                                        "ORA image version or property is not qualified",
                                    ));
                                }
                            }
                        }
                        if !version || empty {
                            return Err(malformed("ORA image lacks version or root stack"));
                        }
                        state = 1;
                    }
                    (1, b"stack") => {
                        if !values.is_empty() || empty {
                            return Err(unsupported("ORA requires a nonempty neutral root stack"));
                        }
                        state = 2;
                    }
                    (2, b"layer") => {
                        if layers.len() >= limits.layers {
                            return Err(resource("ORA layer count exceeds allowance"));
                        }
                        layers.push(layer(values, &mut losses)?);
                        state = if empty { 2 } else { 3 };
                    }
                    _ => {
                        return Err(unsupported(
                            "ORA groups, filters, text, transforms or XML extensions are not supported",
                        ));
                    }
                }
            }
            Event::End(end) => {
                state = match (state, end.name().as_ref()) {
                    (3, b"layer") => 2,
                    (2, b"stack") => 4,
                    (4, b"image") => 5,
                    _ => return Err(malformed("Unexpected ORA XML end tag")),
                };
            }
            Event::Text(value) => {
                if !value.as_ref().iter().all(u8::is_ascii_whitespace) {
                    return Err(unsupported("ORA XML contains non-whitespace text"));
                }
            }
            Event::CData(value) => {
                if !value.as_ref().iter().all(u8::is_ascii_whitespace) {
                    return Err(unsupported("ORA XML contains text data"));
                }
            }
            Event::Comment(_) => {}
            Event::Decl(decl) => {
                if state != 0 || declaration || events != 1 {
                    return Err(malformed("Misplaced ORA XML declaration"));
                }
                declaration = true;
                if decl
                    .version()
                    .map_err(|_| malformed("Invalid ORA XML declaration"))?
                    .as_ref()
                    != b"1.0"
                    || decl
                        .encoding()
                        .transpose()
                        .map_err(|_| malformed("Invalid ORA XML encoding"))?
                        .is_some_and(|encoding| !encoding.eq_ignore_ascii_case(b"utf-8"))
                    || decl
                        .standalone()
                        .transpose()
                        .map_err(|_| malformed("Invalid ORA XML standalone flag"))?
                        .is_some()
                {
                    return Err(unsupported("ORA XML declaration is outside UTF-8 XML 1.0"));
                }
            }
            Event::Eof => break,
            _ => {
                return Err(unsupported(
                    "ORA DTD, processing instructions and entities are forbidden",
                ));
            }
        }
    }
    if state != 5 || layers.is_empty() {
        return Err(malformed("Incomplete or empty ORA layer stack"));
    }
    let (width, height) = (
        width.ok_or_else(|| malformed("ORA lacks width"))?,
        height.ok_or_else(|| malformed("ORA lacks height"))?,
    );
    dimensions(width, height, limits)?;
    if title
        .as_ref()
        .is_some_and(|value| value.chars().count() > 4096)
    {
        return Err(resource("ORA title exceeds allowance"));
    }
    let resolution = match (xres, yres) {
        (None, None) => None,
        (Some(x), Some(y)) if x > 0 && y > 0 && x <= i32::MAX as u32 && y <= i32::MAX as u32 => {
            loss(&mut losses, "ora-resolution-ignored");
            Some((x, y))
        }
        _ => {
            return Err(malformed(
                "ORA resolution must be a positive paired signed integer",
            ));
        }
    };
    Ok(Stack {
        width,
        height,
        title,
        resolution,
        layers,
        losses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(xml: &str) -> Result<Stack> {
        parse(
            xml.as_bytes(),
            &Limits::default(),
            &Budget::new(8 << 20),
            &|| Ok(()),
        )
    }
    #[test]
    fn fractional_opacity_is_explicit_and_xml_entities_are_inert() {
        let parsed=read(r#"<image version="0.0.6" w="1" h="1"><stack><layer src="data/a.png" name="A &amp; &#x65E5;" opacity="0.5"/></stack></image>"#).unwrap();
        assert_eq!(parsed.layers[0].name, "A & 日");
        assert_eq!(parsed.layers[0].opacity, 128);
        assert_eq!(parsed.losses, ["ora-opacity-quantized"]);
    }
    #[test]
    fn unsupported_properties_groups_entities_and_invalid_opacity_fail_closed() {
        for body in [
            r#"<stack><layer src="a.png"/></stack>"#,
            r#"<layer src="a.png" composite-op="svg:multiply"/>"#,
            r#"<layer src="a.png" opacity="NaN"/>"#,
            r#"<layer src="a.png" opacity="1.01"/>"#,
            r#"<layer src="a.png" opacity="-0.01"/>"#,
            r#"<layer src="a.png" name="&external;"/>"#,
            r#"<layer src="a.png" x="2147483648"/>"#,
            r#"<layer src="a.png" src="b.png"/>"#,
            r#"<layer src="a.png" selected="true"/>"#,
            r#"<layer src="a.png">paint</layer>"#,
            r#"<text>paint</text>"#,
        ] {
            assert!(
                read(&format!(
                    r#"<image version="0.0.6" w="1" h="1"><stack>{body}</stack></image>"#
                ))
                .is_err(),
                "{body}"
            );
        }
        assert!(read(r#"<!DOCTYPE image [<!ENTITY external SYSTEM "file:///private/test">]><image version="0.0.6" w="1" h="1"><stack><layer src="a.png"/></stack></image>"#).is_err());
    }
}
