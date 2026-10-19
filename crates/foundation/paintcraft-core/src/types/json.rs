//! Strict JSON for the wire contracts: duplicate keys, unpaired surrogates,
//! non-integer numbers, trailing bytes and excessive nesting are all errors.
//! The canonical writer is RFC 8785 (JCS) for this integer-only subset.

use std::collections::BTreeMap;
use std::fmt::Write as _;

const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError(pub String);

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for JsonError {}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
        Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }
}

pub fn parse(bytes: &[u8]) -> Result<Value, JsonError> {
    let text = std::str::from_utf8(bytes).map_err(|_| err("invalid UTF-8"))?;
    let mut p = Parser {
        s: text.as_bytes(),
        text,
        i: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(err("trailing content after JSON value"));
    }
    Ok(v)
}

fn err(m: &str) -> JsonError {
    JsonError(m.to_owned())
}

struct Parser<'a> {
    s: &'a [u8],
    text: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> Result<(), JsonError> {
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            Ok(())
        } else {
            Err(err("invalid literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(err("nesting too deep"));
        }
        match self.s.get(self.i) {
            None => Err(err("unexpected end of input")),
            Some(b'n') => self.eat("null").map(|_| Value::Null),
            Some(b't') => self.eat("true").map(|_| Value::Bool(true)),
            Some(b'f') => self.eat("false").map(|_| Value::Bool(false)),
            Some(b'"') => self.string().map(Value::Str),
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Value::Array(items));
                }
                loop {
                    self.ws();
                    items.push(self.value(depth + 1)?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Value::Array(items));
                        }
                        _ => return Err(err("expected ',' or ']'")),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut map = BTreeMap::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Value::Object(map));
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&b'"') {
                        return Err(err("expected object key"));
                    }
                    let key = self.string()?;
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return Err(err("expected ':'"));
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value(depth + 1)?;
                    if map.insert(key.clone(), v).is_some() {
                        return Err(JsonError(format!("duplicate JSON key: {key}")));
                    }
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Object(map));
                        }
                        _ => return Err(err("expected ',' or '}'")),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(err("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        if self.s[self.i] == b'-' {
            self.i += 1;
        }
        match self.s.get(self.i) {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
            }
            _ => return Err(err("invalid number")),
        }
        if matches!(self.s.get(self.i), Some(b'.' | b'e' | b'E')) {
            return Err(err(
                "non-integer numbers are not part of the wire contracts",
            ));
        }
        self.text[start..self.i]
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| err("integer out of range"))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let h = self
            .text
            .get(self.i..self.i + 4)
            .ok_or_else(|| err("short \\u escape"))?;
        if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err("invalid \\u escape"));
        }
        self.i += 4;
        Ok(u32::from_str_radix(h, 16).expect("validated hex"))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let Some(&b) = self.s.get(self.i) else {
                return Err(err("unterminated string"));
            };
            match b {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                0..=0x1f => return Err(err("control character in string")),
                b'\\' => {
                    self.i += 1;
                    let Some(&e) = self.s.get(self.i) else {
                        return Err(err("unterminated escape"));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = match hi {
                                0xD800..=0xDBFF => {
                                    if !self.s[self.i..].starts_with(b"\\u") {
                                        return Err(err("unpaired surrogate in JSON string"));
                                    }
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..=0xDFFF).contains(&lo) {
                                        return Err(err("unpaired surrogate in JSON string"));
                                    }
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                }
                                0xDC00..=0xDFFF => {
                                    return Err(err("unpaired surrogate in JSON string"));
                                }
                                other => other,
                            };
                            out.push(char::from_u32(cp).expect("valid scalar"));
                        }
                        _ => return Err(err("invalid escape")),
                    }
                }
                _ => {
                    // Copy one full UTF-8 scalar; `text` is valid UTF-8.
                    let ch = self.text[self.i..].chars().next().expect("in bounds");
                    out.push(ch);
                    self.i += ch.len_utf8();
                }
            }
        }
    }
}

/// RFC 8785 canonical form: sorted by UTF-16 code units, no whitespace, raw UTF-8.
pub fn canonical(value: &Value) -> Vec<u8> {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out.into_bytes()
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(n) => {
            let _ = write!(out, "{n}");
        }
        Value::Str(s) => write_string(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_canonical(&m[k], out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Value, JsonError> {
        parse(s.as_bytes())
    }

    #[test]
    fn canonical_matches_the_oracle_cases() {
        let v = p(r#"{"b":1,"a":["é","\t\u0001\"\\"],"c":null}"#).unwrap();
        assert_eq!(
            String::from_utf8(canonical(&v)).unwrap(),
            "{\"a\":[\"é\",\"\\t\\u0001\\\"\\\\\"],\"b\":1,\"c\":null}"
        );
        // UTF-16 order: U+1F600 (surrogate D83D..) sorts before U+E000.
        let v = p("{\"\u{e000}\":1,\"\u{1f600}\":2}").unwrap();
        assert_eq!(
            String::from_utf8(canonical(&v)).unwrap(),
            "{\"😀\":2,\"\u{e000}\":1}"
        );
    }

    #[test]
    fn rejects_what_the_contracts_forbid() {
        for bad in [
            r#"{"a":1,"a":2}"#,
            r#""\ud800""#,
            r#""\udc00x""#,
            "1.5",
            "1e3",
            "01",
            "-",
            "[1,]",
            "{\"a\":1} x",
            "\"\u{1}\"",
            "NaN",
            "99999999999999999999",
            "",
            "{\"a\" 1}",
        ] {
            assert!(p(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(parse(&[0x22, 0xff, 0x22]).is_err());
        assert!(p(&format!("{}1{}", "[".repeat(100), "]".repeat(100))).is_err());
    }

    #[test]
    fn accepts_pairs_and_whitespace() {
        assert_eq!(p(r#""😀""#).unwrap(), Value::str("😀"));
        assert_eq!(p(" [ 1 , -2 ,0 ]\n").unwrap().as_array().unwrap().len(), 3);
        assert_eq!(p("-9223372036854775808").unwrap(), Value::Int(i64::MIN));
    }
}
