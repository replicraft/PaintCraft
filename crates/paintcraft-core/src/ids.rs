//! Deterministic identifiers: name-based UUIDv5 in a fixed namespace.
//! Names are `/`-joined ASCII fields; see the determinism contract.

use uuid::Uuid;

/// `uuid5(NAMESPACE_URL, "urn:paintcraft:deterministic-id:v1")`.
pub fn namespace() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"urn:paintcraft:deterministic-id:v1")
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
