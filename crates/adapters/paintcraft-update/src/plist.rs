//! Just enough of the XML property list format to read the string keys of an `Info.plist` the
//! release packager wrote. Anything it does not understand is an error, never a guess.

/// The text of `<key>NAME</key><string>VALUE</string>`, with only whitespace between the two.
pub fn string_value(xml: &str, name: &str) -> Option<String> {
    if !xml.contains("<plist") {
        return None;
    }
    let marker = format!("<key>{name}</key>");
    let mut from = 0;
    while let Some(at) = xml[from..].find(&marker) {
        let rest = xml[from + at + marker.len()..].trim_start();
        if let Some(body) = rest.strip_prefix("<string>") {
            let end = body.find("</string>")?;
            let value = &body[..end];
            return (!value.contains(['<', '&'])).then(|| value.to_owned());
        }
        from += at + marker.len();
    }
    None
}
