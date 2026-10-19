//! Installed family enumeration from standard system and user font locations.
#[derive(Clone, Debug, Default)]
pub struct FontCatalog {
    pub families: Vec<String>,
}

impl FontCatalog {
    pub fn system() -> Self {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        Self::from_database(&database)
    }

    pub fn from_database(database: &fontdb::Database) -> Self {
        let mut names = std::collections::BTreeMap::new();
        for face in database.faces() {
            if let Some((name, _)) = face.families.first()
                && !name.is_empty()
                && !name.starts_with('.')
            {
                names
                    .entry(name.to_lowercase())
                    .or_insert_with(|| name.clone());
            }
        }
        Self {
            families: names.into_values().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an installed system font collection; native capability review"]
    fn installed_font_catalog_for_review() {
        let catalog = FontCatalog::system();
        assert!(!catalog.families.is_empty());
        eprintln!(
            "Enumerated {} installed font families; first ten: {:?}",
            catalog.families.len(),
            &catalog.families[..catalog.families.len().min(10)]
        );
    }

    #[test]
    fn catalog_reads_real_family_metadata_and_deduplicates_faces() {
        let mut database = fontdb::Database::new();
        let bytes = include_bytes!("../../paintcraft-ui/assets/selawik/selawk.ttf");
        database.load_font_data(bytes.to_vec());
        database.load_font_data(bytes.to_vec());
        assert_eq!(
            FontCatalog::from_database(&database).families,
            vec!["Selawik"]
        );
    }
}
