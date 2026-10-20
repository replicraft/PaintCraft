use super::support::{fixture, verify};
use paintcraft_document::Document;
use paintcraft_render::{
    parallel::{Partition, render_in_pool},
    render_flat,
};

pub const CASES: &[&str] = &[
    "tiny",
    "opaque1",
    "sparse5",
    "sparse20",
    "sparse100",
    "dense5",
    "clipped100",
];
pub const VARIANTS: &[&str] = &[
    "serial",
    "bands64-t1",
    "bands64-t2",
    "bands64-t4",
    "bands256-t4",
    "tiles64-t4",
    "tiles256-t4",
];
pub struct Runner {
    pool: Option<rayon::ThreadPool>,
    partition: Partition,
}
impl Runner {
    pub fn new(variant: &str) -> Self {
        assert!(VARIANTS.contains(&variant));
        if variant == "serial" {
            return Self {
                pool: None,
                partition: Partition::Bands { rows: 64 },
            };
        }
        let (kind, threads) = variant.split_once("-t").unwrap();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.parse().unwrap())
            .build()
            .unwrap();
        let partition = if let Some(size) = kind.strip_prefix("bands") {
            Partition::Bands {
                rows: size.parse().unwrap(),
            }
        } else {
            Partition::Tiles {
                edge: kind.strip_prefix("tiles").unwrap().parse().unwrap(),
            }
        };
        Self {
            pool: Some(pool),
            partition,
        }
    }
    pub fn render(&self, doc: &Document) -> Vec<u8> {
        match &self.pool {
            None => render_flat(doc),
            Some(pool) => render_in_pool(
                pool,
                doc,
                doc.canvas_rect(),
                self.partition,
                32 << 20,
                || Ok(()),
            ),
        }
        .unwrap()
    }
    pub fn verify(case: &str) {
        let doc = fixture(case);
        verify(&doc); // Frozen row-copy baseline, independent from partition mechanics.
        let expected = render_flat(&doc).unwrap();
        for variant in VARIANTS {
            assert_eq!(
                Self::new(variant).render(&doc),
                expected,
                "{case}/{variant}"
            );
        }
    }
}

/// Cheap liveness guard, distinct from the full independent correctness preflight.
pub struct Guard {
    bytes: usize,
    probes: [(usize, [u8; 4]); 5],
}
impl Guard {
    pub fn new(doc: &Document) -> Self {
        let w = i64::from(doc.canvas.0);
        let h = i64::from(doc.canvas.1);
        let probes = [
            (0, 0),
            (w - 1, h - 1),
            (w / 2, h / 2),
            (31.min(w - 1), 47.min(h - 1)),
            (62.min(w - 1), 94.min(h - 1)),
        ]
        .map(|(x, y)| {
            let mut color = paintcraft_core::types::Rgba::TRANSPARENT;
            for layer in doc
                .layers_in_order()
                .filter(|layer| layer.visible && layer.extent().contains(x, y))
            {
                color = layer
                    .raster
                    .get(x - layer.origin.0, y - layer.origin.1)
                    .with_opacity(layer.opacity)
                    .source_over(color);
            }
            (((y * w + x) * 4) as usize, color.0)
        });
        Self {
            bytes: (w * h * 4) as usize,
            probes,
        }
    }
    pub fn check(&self, pixels: &[u8]) {
        assert_eq!(pixels.len(), self.bytes, "complete output required");
        for (offset, color) in self.probes {
            assert_eq!(pixels[offset..offset + 4], color, "live output probe");
        }
    }
}
