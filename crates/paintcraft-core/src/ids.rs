//! Deterministic identifiers: name-based UUIDv5 in a fixed namespace.
//! Names are `/`-joined ASCII fields; see the determinism contract.

use uuid::Uuid;

/// `uuid5(NAMESPACE_URL, "urn:paintcraft:deterministic-id:v1")`.
pub fn namespace() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"urn:paintcraft:deterministic-id:v1")
}

/// Wire-format UUID: 36 characters, hyphens at 8/13/18/23, hex elsewhere (either case).
pub fn parse_uuid(s: &str) -> Option<Uuid> {
    let ok = s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        });
    if ok { Uuid::parse_str(s).ok() } else { None }
}

pub fn deterministic_id(parts: &[&str]) -> Uuid {
    Uuid::new_v5(&namespace(), parts.join("/").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_is_the_published_constant() {
        assert_eq!(
            namespace().to_string(),
            "5b651efa-6cf1-5532-aa7d-32b74b648ae5"
        );
    }

    #[test]
    fn wire_uuid_grammar_is_strict() {
        assert!(parse_uuid("11111111-1111-4111-8111-111111111111").is_some());
        assert!(parse_uuid("AAAAAAAA-1111-4111-8111-111111111111").is_some());
        for bad in [
            "",
            "11111111111141118111111111111111",
            "11111111-1111-4111-8111-11111111111g",
            "{11111111-1111-4111-8111-111111111111}",
            "urn:uuid:11111111-1111-4111-8111-111111111111",
        ] {
            assert!(parse_uuid(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn same_name_same_id_different_name_different_id() {
        assert_eq!(
            deterministic_id(&["layer", "a", "0"]),
            deterministic_id(&["layer", "a", "0"])
        );
        assert_ne!(
            deterministic_id(&["layer", "a", "0"]),
            deterministic_id(&["layer", "a", "1"])
        );
    }
}
