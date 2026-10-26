//! The persistent document: canvas, ordered flat layer roots and raster content.
//! Selection, tools and history are session state and live elsewhere.

use std::collections::HashMap;

use paintcraft_core::types::json::{self, Value};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use paintcraft_core::types::ids::deterministic_id;
use paintcraft_raster::RasterLayer;

pub use paintcraft_core::types::policy::CANVAS_SIDE_MAX;
pub use paintcraft_core::types::policy::COORD_ABS_MAX;
pub use paintcraft_core::types::policy::LAYER_COUNT_MAX;
pub use paintcraft_core::types::policy::POLICY;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub id: Uuid,
    pub name: String,
    /// Document-space position of the layer's local (0,0).
    pub origin: (i64, i64),
    pub visible: bool,
    pub opacity: u8,
    pub raster: RasterLayer,
}

impl Layer {
    /// Document-space rectangle covered by the layer.
    pub fn extent(&self) -> Rect {
        Rect::from_origin_size(
            self.origin.0,
            self.origin.1,
            self.raster.width(),
            self.raster.height(),
        )
        .expect("origin is range-checked at admission")
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub document_id: Uuid,
    pub state_id: Uuid,
    pub canvas: (u32, u32),
    /// Bottom to top.
    pub roots: Vec<Uuid>,
    pub layers: HashMap<Uuid, Layer>,
    pub title: Option<String>,
}

impl Document {
    pub fn canvas_rect(&self) -> Rect {
        Rect::from_origin_size(0, 0, self.canvas.0, self.canvas.1).expect("canvas is non-empty")
    }

    pub fn layer(&self, id: &Uuid) -> Option<&Layer> {
        self.layers.get(id)
    }

    /// Roots in stacking order, bottom to top.
    pub fn layers_in_order(&self) -> impl Iterator<Item = &Layer> {
        self.roots.iter().map(|id| &self.layers[id])
    }

    /// Persisted meaning, excluding the StateId itself.
    pub fn same_semantics(&self, other: &Document) -> bool {
        self.document_id == other.document_id
            && self.canvas == other.canvas
            && self.roots == other.roots
            && self.title == other.title
            && self
                .roots
                .iter()
                .all(|id| self.layers[id] == other.layers[id])
    }
}

/// The shared document factory behind `paintcraft-cli new`: a transparent
/// document with no layers, or one full-canvas `Background` layer when a
/// background colour is requested.
pub fn new_document(
    width: u32,
    height: u32,
    background: Option<Rgba>,
    seed: u64,
    policy: &str,
) -> Result<Document> {
    if !(1..=CANVAS_SIDE_MAX).contains(&width) || !(1..=CANVAS_SIDE_MAX).contains(&height) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "canvas dimensions out of range",
        ));
    }
    if seed > 9_007_199_254_740_991 {
        return Err(Error::new(ErrorCode::InputSchema, "seed out of range"));
    }
    if policy != POLICY {
        return Err(Error::new(
            ErrorCode::PolicyUnknown,
            format!("policy {policy:?} is not implemented"),
        ));
    }
    let params = Value::object([
        (
            "background",
            background.map_or(Value::Null, |c| Value::str(c.to_hex())),
        ),
        ("height", Value::Int(i64::from(height))),
        ("policy", Value::str(policy)),
        ("seed", Value::Int(seed as i64)),
        ("width", Value::Int(i64::from(width))),
    ]);
    let digest = hex(&Sha256::digest(json::canonical(&params)));
    let mut layers = HashMap::new();
    let mut roots = Vec::new();
    if let Some(color) = background {
        let id = deterministic_id(&["layer", "new", &digest, "0"]);
        layers.insert(
            id,
            Layer {
                id,
                name: "Background".to_owned(),
                origin: (0, 0),
                visible: true,
                opacity: 255,
                raster: RasterLayer::new(width, height, color),
            },
        );
        roots.push(id);
    }
    Ok(Document {
        document_id: deterministic_id(&["document", &digest]),
        state_id: deterministic_id(&["state", "new", &digest]),
        canvas: (width, height),
        roots,
        layers,
        title: None,
    })
}

/// A document holding one imported picture: the canvas is the picture's size and
/// a single `Image` layer carries every sample exactly (hidden RGB included).
/// `source_sha256` is the digest of the file imported, so the same file always
/// yields the same identifiers.
pub fn import_image(width: u32, height: u32, rgba: &[u8], source_sha256: &str) -> Result<Document> {
    if !(1..=CANVAS_SIDE_MAX).contains(&width) || !(1..=CANVAS_SIDE_MAX).contains(&height) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "image dimensions out of range",
        ));
    }
    let mut adm =
        paintcraft_raster::Admission::new(paintcraft_core::types::policy::STAGED_PIXELS_MAX);
    let raster = RasterLayer::from_rgba(width, height, Rgba::TRANSPARENT, rgba, &mut adm)?;
    Ok(imported_document(width, height, raster, source_sha256))
}

/// Construct private imported pixels with cancellation throughout tile admission
/// and copying. The caller retains its resource lease through publication.
pub fn import_image_checked(
    width: u32,
    height: u32,
    rgba: &[u8],
    source_sha256: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    check()?;
    if !(1..=CANVAS_SIDE_MAX).contains(&width) || !(1..=CANVAS_SIDE_MAX).contains(&height) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "image dimensions out of range",
        ));
    }
    let mut admission =
        paintcraft_raster::Admission::new(paintcraft_core::types::policy::STAGED_PIXELS_MAX);
    let raster = RasterLayer::from_rgba_checked(
        width,
        height,
        Rgba::TRANSPARENT,
        rgba,
        &mut admission,
        check,
    )?;
    check()?;
    Ok(imported_document(width, height, raster, source_sha256))
}
fn imported_document(
    width: u32,
    height: u32,
    raster: RasterLayer,
    source_sha256: &str,
) -> Document {
    let id = deterministic_id(&["layer", "import", source_sha256]);
    let layer = Layer {
        id,
        name: "Image".to_owned(),
        origin: (0, 0),
        visible: true,
        opacity: 255,
        raster,
    };
    Document {
        document_id: deterministic_id(&["document", "import", source_sha256]),
        state_id: deterministic_id(&["state", "import", source_sha256]),
        canvas: (width, height),
        roots: vec![id],
        layers: HashMap::from([(id, layer)]),
        title: None,
    }
}

pub use paintcraft_core::types::policy::hex;

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgba = Rgba([255, 255, 255, 255]);

    #[test]
    fn default_is_transparent_with_no_layers() {
        let d = new_document(5, 4, None, 0, POLICY).unwrap();
        assert!(d.roots.is_empty() && d.layers.is_empty());
        assert_eq!(d.canvas, (5, 4));
    }

    #[test]
    fn requested_background_is_one_full_canvas_layer_without_blocks() {
        let d = new_document(5, 4, Some(WHITE), 0, POLICY).unwrap();
        let layer = d.layer(&d.roots[0]).unwrap();
        assert_eq!(
            (
                layer.name.as_str(),
                layer.origin,
                layer.opacity,
                layer.visible
            ),
            ("Background", (0, 0), 255, true)
        );
        assert_eq!(
            (
                layer.raster.width(),
                layer.raster.height(),
                layer.raster.default_pixel()
            ),
            (5, 4, WHITE)
        );
        assert!(layer.raster.blocks().is_empty());
        assert_eq!(layer.extent(), d.canvas_rect());
    }

    #[test]
    fn ids_are_deterministic_and_parameter_sensitive() {
        let a = new_document(5, 4, Some(WHITE), 0, POLICY).unwrap();
        let b = new_document(5, 4, Some(WHITE), 0, POLICY).unwrap();
        assert_eq!(
            (a.document_id, a.state_id, a.roots.clone()),
            (b.document_id, b.state_id, b.roots)
        );
        assert_ne!(
            new_document(5, 4, Some(WHITE), 1, POLICY)
                .unwrap()
                .document_id,
            a.document_id
        );
        assert_ne!(
            new_document(5, 4, None, 0, POLICY).unwrap().document_id,
            a.document_id
        );
    }

    #[test]
    fn imported_pictures_keep_every_sample_and_have_stable_identity() {
        let px: Vec<u8> = (0..6 * 4u8).collect();
        let a = import_image(3, 2, &px, &"a".repeat(64)).unwrap();
        let b = import_image(3, 2, &px, &"a".repeat(64)).unwrap();
        assert_eq!(
            (a.document_id, a.state_id, &a.roots),
            (b.document_id, b.state_id, &b.roots)
        );
        assert_ne!(
            import_image(3, 2, &px, &"b".repeat(64))
                .unwrap()
                .document_id,
            a.document_id
        );
        let mut rows = Vec::new();
        a.layers[&a.roots[0]]
            .raster
            .for_each_row(|r| rows.extend_from_slice(r));
        assert_eq!(rows, px);
        assert_eq!(a.canvas, (3, 2));
        assert!(import_image(3, 2, &px[1..], "x").is_err());
    }

    #[test]
    fn invalid_requests_have_stable_codes() {
        let code = |r: Result<Document>| r.unwrap_err().code;
        assert_eq!(
            code(new_document(0, 2, None, 0, POLICY)),
            ErrorCode::InputSchema
        );
        assert_eq!(
            code(new_document(2, CANVAS_SIDE_MAX + 1, None, 0, POLICY)),
            ErrorCode::InputSchema
        );
        assert_eq!(
            code(new_document(2, 2, None, 1 << 60, POLICY)),
            ErrorCode::InputSchema
        );
        assert_eq!(
            code(new_document(2, 2, None, 0, "paintcraft.core/missing-v1")),
            ErrorCode::PolicyUnknown
        );
    }

    #[test]
    fn same_semantics_ignores_state_id_only() {
        let a = new_document(2, 2, Some(WHITE), 0, POLICY).unwrap();
        let mut b = a.clone();
        b.state_id = Uuid::nil();
        assert!(a.same_semantics(&b));
        b.layers.get_mut(&b.roots[0].clone()).unwrap().opacity = 7;
        assert!(!a.same_semantics(&b));
    }
}

#[cfg(test)]
mod import_tests;
