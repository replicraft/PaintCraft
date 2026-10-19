//! Layer commands select new targets only after successful publication.
use super::*;

impl Editor {
    pub(super) fn add_layer(&mut self) {
        let doc = self.document();
        let (w, h) = doc.canvas;
        let name = format!("Layer {}", doc.roots.len() + 1);
        let op = Value::object([
            ("op", Value::str("layer.add")),
            ("ref", Value::str("layer")),
            ("name", Value::str(&name)),
            ("width", Value::Int(i64::from(w))),
            ("height", Value::Int(i64::from(h))),
            (
                "origin",
                Value::object([("x", Value::Int(0)), ("y", Value::Int(0))]),
            ),
            ("default_pixel", Value::str("#00000000")),
        ]);
        if let Some(r) = self.commit(vec![op], &format!("Added {name}")) {
            // The new layer becomes the target only after a successful commit.
            self.target = r.aliases.get("layer").copied().or(self.target);
        }
    }

    pub(super) fn duplicate_layer(&mut self, layer: Uuid) {
        let Some(source) = self.document().layer(&layer) else {
            self.status = "Cannot duplicate: layer is no longer present".into();
            return;
        };
        let name = format!(
            "{} Copy",
            source.name.chars().take(1019).collect::<String>()
        );
        let op = Value::object([
            ("op", Value::str("layer.duplicate")),
            ("layer_ref", layer_ref(layer)),
            ("ref", Value::str("copy")),
            ("name", Value::str(name)),
        ]);
        self.cancel_gesture();
        if let Some(receipt) = self.commit(vec![op], "Layer duplicated") {
            self.target = receipt.aliases.get("copy").copied().or(self.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_long_unicode_name_remains_valid_and_failed_copy_retains_target() {
        let mut e = Editor::new_document(4, 4).unwrap();
        let source = e.target().unwrap();
        e.request(
            1,
            Action::Rename {
                layer: source,
                name: "界".repeat(1024),
            },
        );
        e.run_pending();
        let before = e.publications();
        e.request(2, Action::DuplicateLayer { layer: source });
        e.run_pending();
        let copy = e.target().unwrap();
        assert_ne!(copy, source);
        assert_eq!(e.publications(), before + 1);
        assert_eq!(e.document().layers[&copy].name.chars().count(), 1024);
        assert!(e.document().layers[&copy].name.ends_with(" Copy"));
        e.request(3, Action::DuplicateLayer { layer: Uuid::nil() });
        e.run_pending();
        assert_eq!(e.target(), Some(copy));
        assert_eq!(e.publications(), before + 1);
    }
}
