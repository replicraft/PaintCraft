//! Independent dense traversal observes earlier masked writes and frozen copies.
use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Rect, Rgba, resources::SharedBudget, text::MaskSpan};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::execute::{Selection, execute};
use paintcraft_raster::{
    Admission,
    selection::{BinaryMask, MaskLimits},
};
use std::collections::VecDeque;
const W: usize = 6;
const H: usize = 5;
const WHITE: Rgba = Rgba([255; 4]);
const WALL: Rgba = Rgba([13, 55, 97, 255]);
const RED: Rgba = Rgba([211, 29, 73, 255]);
fn inside(x: i64, y: i64) -> bool {
    (0..W as i64).contains(&x) && (0..H as i64).contains(&y)
}
#[test]
fn compiled_flood_obeys_mask_connectivity_after_prior_paint_and_duplicate() {
    for origin in [(-1, 1), (1, -1)] {
        for case in 0..48 {
            for seed in [(0, 0), (1, 1), (3, 3), (5, 4)] {
                let mut doc = new_document(W as u32, H as u32, Some(WHITE), 0, POLICY).unwrap();
                let id = doc.roots[0];
                doc.layers.get_mut(&id).unwrap().origin = origin;
                let mut pixels = vec![WHITE; W * H];
                let mut allowed = [false; W * H];
                let mut spans = Vec::new();
                let mut admission = Admission::new(65536);
                for y in 0..H {
                    for x in 0..W {
                        let value = if (x * 11 + y * 7 + case) % 5 == 0 {
                            WALL
                        } else {
                            WHITE
                        };
                        pixels[y * W + x] = value;
                        doc.layers
                            .get_mut(&id)
                            .unwrap()
                            .raster
                            .blend_pixel(x as i64, y as i64, value, Mode::Replace, &mut admission)
                            .unwrap();
                    }
                }
                // Finite document-domain mask, intentionally disconnected in many cases.
                for y in 0..H {
                    for x in 0..W {
                        let selected =
                            case == 0 || ((x * 13 + y * 17 + case * 19) ^ (case >> 1)) % 7 < 4;
                        allowed[y * W + x] = selected;
                        if selected {
                            spans.push(MaskSpan {
                                x0: x as i64,
                                x1: x as i64,
                                y: y as i64,
                                coverage: 255,
                            });
                        }
                    }
                }
                let mask = BinaryMask::from_spans(
                    doc.canvas_rect(),
                    false,
                    &spans,
                    &SharedBudget::new(1 << 20),
                    MaskLimits::default(),
                    &|| Ok(()),
                )
                .unwrap();
                let eligible = |lx: i64, ly: i64| {
                    let (x, y) = (lx + origin.0, ly + origin.1);
                    inside(lx, ly) && inside(x, y) && allowed[y as usize * W + x as usize]
                };
                // Earlier compiled replacement wall, only inside the current selection.
                for y in 0..H {
                    for x in 0..W {
                        if eligible(x as i64, y as i64) && x as i64 + origin.0 == 2 {
                            pixels[y * W + x] = WALL;
                        }
                    }
                }
                let before_flood = pixels.clone();
                let mut visited = [false; W * H];
                let local = (seed.0 - origin.0, seed.1 - origin.1);
                if eligible(local.0, local.1) {
                    let target = pixels[local.1 as usize * W + local.0 as usize];
                    let mut q = VecDeque::from([local]);
                    while let Some((x, y)) = q.pop_front() {
                        if !eligible(x, y) {
                            continue;
                        }
                        let i = y as usize * W + x as usize;
                        if visited[i] || before_flood[i] != target {
                            continue;
                        }
                        visited[i] = true;
                        pixels[i] = RED;
                        q.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
                    }
                }
                let b = Batch {
                    version: 2,
                    policy: POLICY.into(),
                    base_state: doc.state_id,
                    base_epoch: 0,
                    seed: None,
                    digest: "8".repeat(64),
                    operations: vec![
                        Op::FillRect {
                            layer: LayerRef::Id(id),
                            rect: Rect::new(2, 0, 1, H as i64).unwrap(),
                            color: WALL,
                            mode: Mode::Replace,
                        },
                        Op::LayerDuplicate {
                            layer: LayerRef::Id(id),
                            alias: "copy".into(),
                            name: "Copy".into(),
                        },
                        Op::FloodFill {
                            layer: LayerRef::Alias("copy".into()),
                            at: seed,
                            color: RED,
                            mode: Mode::Replace,
                            tolerance: 0,
                        },
                    ],
                };
                let out = execute(&doc, Selection::Mask(mask), &b).unwrap();
                let copy = out.aliases["copy"];
                for y in 0..H {
                    for x in 0..W {
                        assert_eq!(
                            out.doc.layers[&id].raster.get(x as i64, y as i64),
                            before_flood[y * W + x],
                            "original case{case}"
                        );
                        assert_eq!(
                            out.doc.layers[&copy].raster.get(x as i64, y as i64),
                            pixels[y * W + x],
                            "copy case{case} seed{seed:?} origin{origin:?} local({x},{y})"
                        );
                    }
                }
            }
        }
    }
}
