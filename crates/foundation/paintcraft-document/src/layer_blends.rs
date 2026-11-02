//! Stored blend intent is independent from visibility and apparent neutrality.
use super::*;
impl Document {
    pub fn has_non_normal_blends(&self) -> bool {
        self.layers
            .values()
            .any(|l| l.blend_mode != LayerBlendMode::Normal)
    }
    /// Unqualified consumers must not silently treat a stored mode as Normal.
    pub fn require_normal_blends(&self) -> Result<()> {
        if self.has_non_normal_blends() {
            Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Layer blend modes require a qualified appearance route",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_semantics_and_target_observed_public_layout() {
        let doc = new_document(1, 1, Some(Rgba([17, 32, 255, 0])), 0, POLICY).unwrap();
        let id = doc.roots[0];
        assert_eq!(doc.layers[&id].blend_mode, LayerBlendMode::Normal);
        assert!(!doc.has_non_normal_blends());
        assert!(!doc.requires_flatten());
        let mut changed = doc.clone();
        changed.layers.get_mut(&id).unwrap().blend_mode = LayerBlendMode::Multiply;
        changed.layers.get_mut(&id).unwrap().visible = false;
        changed.layers.get_mut(&id).unwrap().opacity = 0;
        assert!(changed.has_non_normal_blends());
        assert!(changed.requires_flatten());
        assert!(!doc.same_semantics(&changed));
        assert_eq!(
            changed.require_normal_blends().unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        eprintln!(
            "BLEND_PUBLIC_LAYOUT arch={} pointer={} layer={} document={} mode={}",
            std::env::consts::ARCH,
            std::mem::size_of::<usize>(),
            std::mem::size_of::<Layer>(),
            std::mem::size_of::<Document>(),
            std::mem::size_of::<LayerBlendMode>()
        );
    }
}
