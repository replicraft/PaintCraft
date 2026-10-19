//! Minimal, strict argument parsing: every flag a command accepts is declared,
//! anything else is an invocation error (exit status 2).

use std::collections::HashMap;

use paintcraft_types::{Error, ErrorCode, Result};

#[derive(Debug)]
pub struct Args {
    pub positional: Vec<String>,
    values: HashMap<&'static str, String>,
    switches: Vec<&'static str>,
}

fn usage(m: impl Into<String>) -> Error {
    Error::new(ErrorCode::InputSchema, m)
}

impl Args {
    /// `value_flags` take an argument, `switches` do not. `-o` is `--output`.
    pub fn parse(
        raw: &[String],
        value_flags: &[&'static str],
        switches: &[&'static str],
    ) -> Result<Args> {
        let mut out = Args {
            positional: Vec::new(),
            values: HashMap::new(),
            switches: Vec::new(),
        };
        let mut i = 0;
        while i < raw.len() {
            let a = raw[i].as_str();
            i += 1;
            let Some(flag) = (if a == "-o" {
                Some("--output")
            } else if a.starts_with("--") {
                Some(a)
            } else {
                None
            }) else {
                out.positional.push(a.to_owned());
                continue;
            };
            let (name, inline) = match flag.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (flag, None),
            };
            let name = name.trim_start_matches("--");
            if let Some(known) = switches.iter().find(|s| **s == name) {
                if inline.is_some() {
                    return Err(usage(format!("--{name} takes no value")));
                }
                out.switches.push(known);
            } else if let Some(known) = value_flags.iter().find(|s| **s == name) {
                let value = match inline {
                    Some(v) => v,
                    None => {
                        i += 1;
                        raw.get(i - 1)
                            .cloned()
                            .ok_or_else(|| usage(format!("--{name} needs a value")))?
                    }
                };
                if out.values.insert(known, value).is_some() {
                    return Err(usage(format!("--{name} given twice")));
                }
            } else {
                return Err(usage(format!("unknown option --{name}")));
            }
        }
        Ok(out)
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn required(&self, name: &str) -> Result<&str> {
        self.value(name)
            .ok_or_else(|| usage(format!("--{name} is required")))
    }

    pub fn flag(&self, name: &str) -> bool {
        self.switches.contains(&name)
    }

    pub fn number<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>> {
        self.value(name)
            .map(|v| {
                v.parse()
                    .map_err(|_| usage(format!("--{name} must be a non-negative integer")))
            })
            .transpose()
    }

    /// Exactly `n` positional arguments.
    pub fn exactly(&self, n: usize, what: &str) -> Result<&[String]> {
        if self.positional.len() == n {
            Ok(&self.positional)
        } else {
            Err(usage(format!("expected {what}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Args> {
        Args::parse(
            &a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            &["width", "output"],
            &["json", "overwrite"],
        )
    }

    #[test]
    fn parses_values_switches_aliases_and_positionals() {
        let a = p(&[
            "in.pcnd",
            "--width",
            "5",
            "-o",
            "out",
            "--json",
            "--output=ignored-dup",
        ])
        .unwrap_err();
        assert_eq!(
            a.code,
            ErrorCode::InputSchema,
            "duplicate --output via -o and --output="
        );
        let a = p(&["in.pcnd", "--width=5", "-o", "out", "--json"]).unwrap();
        assert_eq!(
            (
                a.positional.as_slice(),
                a.value("width"),
                a.value("output"),
                a.flag("json"),
                a.flag("overwrite")
            ),
            (
                &["in.pcnd".to_owned()][..],
                Some("5"),
                Some("out"),
                true,
                false
            )
        );
        assert_eq!(a.number::<u32>("width").unwrap(), Some(5));
    }

    #[test]
    fn rejects_unknown_missing_and_malformed() {
        for bad in [
            &["--nope"][..],
            &["--width"],
            &["--json=1"],
            &["--width", "1", "--width", "2"],
        ] {
            assert_eq!(
                p(bad).err().map(|e| e.code),
                Some(ErrorCode::InputSchema),
                "{bad:?}"
            );
        }
        let a = p(&["--width", "-3"]).unwrap();
        assert!(a.number::<u32>("width").is_err());
        assert!(a.required("output").is_err());
        assert!(a.exactly(1, "one input").is_err());
    }
}
