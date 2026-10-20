//! Same admitted edits, history retention and exact output for all cache contenders.
use super::support::fixture;
use paintcraft_core::types::Rect;
use paintcraft_document::POLICY;
use paintcraft_editor::session::Session;
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    render_region,
};

pub const CASES: &[&str] = &[
    "sparse1",
    "sparse5",
    "sparse20",
    "sparse100",
    "dense5",
    "global5",
    "global-bottom5",
    "viewport100",
];
pub const VARIANTS: &[&str] = &["full", "tile64", "tile128", "tile256"];

pub struct Replay {
    session: Session,
    cache: Option<CompositeCache>,
    next: u32,
    case: String,
    region: Rect,
}
impl Replay {
    pub fn new(case: &str, variant: &str) -> Self {
        assert!(CASES.contains(&case));
        assert!(VARIANTS.contains(&variant));
        let doc = fixture(match case {
            "global5" | "global-bottom5" => "uniform5",
            "viewport100" => "sparse100",
            other => other,
        });
        let region = if case == "viewport100" {
            Rect::new(25, 19, 384, 256).unwrap()
        } else {
            doc.canvas_rect()
        };
        let cache = match variant {
            "full" => None,
            other => Some(
                CompositeCache::new(CacheLimits {
                    tile_edge: other[4..].parse().unwrap(),
                    committed_bytes: 16 << 20,
                    preview_bytes: 4 << 20,
                    entries_per_domain: 512,
                })
                .unwrap(),
            ),
        };
        let mut session = Session::new(doc);
        // Dense fixtures already exceed the former soft 32 MiB floor. Retain the
        // complete required timeline under the preserving history policy.
        session.set_history_budget(768 << 20);
        let mut replay = Self {
            session,
            cache,
            next: 0,
            case: case.to_owned(),
            region,
        };
        // Equal source-page/output warmup for the baseline, and explicit cold-cache
        // preparation for the contenders. Process timings include this first frame.
        std::hint::black_box(replay.render());
        replay
    }
    fn render(&mut self) -> Vec<u8> {
        match &mut self.cache {
            Some(cache) => {
                cache
                    .render(
                        self.session.document(),
                        self.region,
                        Domain::Committed,
                        || Ok(()),
                    )
                    .unwrap()
                    .pixels
            }
            None => render_region(self.session.document(), self.region).unwrap(),
        }
    }
    pub fn step(&mut self) -> Vec<u8> {
        let n = self.next;
        self.next += 1;
        let ticket = self.session.ticket();
        let layer = if self.case == "global-bottom5" {
            self.session.document().roots.first().unwrap()
        } else {
            self.session.document().roots.last().unwrap()
        };
        let operation = if self.case.starts_with("global") {
            format!(
                r#"{{"op":"layer.set_properties","layer_ref":{{"id":"{layer}"}},"opacity_u8":{}}}"#,
                if n.is_multiple_of(2) { 73 } else { 255 }
            )
        } else {
            format!(
                r##"{{"op":"raster.pencil_path","layer_ref":{{"id":"{layer}"}},"points":[{{"x":{},"y":{}}}],"color":"{}","mode":"source-over"}}"##,
                37 + n % 5,
                40 + n % 7,
                if n.is_multiple_of(2) {
                    "#17589CCC"
                } else {
                    "#DA371BBC"
                }
            )
        };
        let json = format!(
            r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{operation}]}}"#,
            ticket.state_id, ticket.publication_epoch
        );
        assert!(self.session.apply_json(json.as_bytes()).unwrap().committed);
        self.render()
    }
    pub fn verify(case: &str, variant: &str) {
        let mut full = Self::new(case, "full");
        let mut candidate = Self::new(case, variant);
        for _ in 0..12 {
            assert_eq!(full.step(), candidate.step());
        }
    }
}
