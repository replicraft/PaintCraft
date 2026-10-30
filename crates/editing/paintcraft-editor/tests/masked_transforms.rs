use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result, Rgba, orthogonal::OrthogonalTransform as T,
    resources::SharedBudget,
};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_editor::{execute::Selection, session::Session};
use paintcraft_raster::{Admission, RasterLayer};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};
use uuid::Uuid;
const ALL: [T; 5] = [
    T::RotateRight,
    T::RotateLeft,
    T::Rotate180,
    T::FlipHorizontal,
    T::FlipVertical,
];
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn batch(s: &Session, ops: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: s.ticket().state_id,
        base_epoch: s.ticket().publication_epoch,
        seed: None,
        digest: format!("{:064x}", s.ticket().publication_epoch + 1),
        operations: ops,
    }
}
fn apply(s: &mut Session, ops: Vec<Op>) {
    s.apply(&batch(s, ops)).unwrap();
}
fn setup(origin: (i64, i64)) -> (Session, Uuid) {
    let mut doc = new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    doc.title = Some("Masked transform fixture".into());
    let id = Uuid::new_v4();
    let mut raster = RasterLayer::new(5, 4, Rgba([11, 22, 33, 0]));
    for y in 0..4 {
        for x in 0..5 {
            if (x + y) % 3 != 0 {
                raster
                    .blend_pixel(
                        x,
                        y,
                        Rgba([
                            x as u8 * 39,
                            y as u8 * 51,
                            91,
                            if (x + y) % 2 == 0 { 0 } else { 128 },
                        ]),
                        Mode::Replace,
                        &mut Admission::new(65_536),
                    )
                    .unwrap();
            }
        }
    }
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "Hidden target 日本語".into(),
            origin,
            visible: false,
            opacity: 73,
            filters: Default::default(),
            raster,
        },
    );
    (Session::new(doc), id)
}
fn polygon() -> Op {
    // Outer 4x3 rectangle, inner 2x1 hole; the bridge is traversed twice.
    Op::SelectionSetPolygon {
        points: vec![
            (4, 3),
            (8, 3),
            (8, 6),
            (4, 6),
            (4, 3),
            (5, 4),
            (5, 5),
            (7, 5),
            (7, 4),
            (5, 4),
            (4, 3),
        ],
    }
}
fn ring() -> BTreeSet<(i64, i64)> {
    (3..6)
        .flat_map(|y| {
            (4..8)
                .filter(move |&x| !((5..7).contains(&x) && y == 4))
                .map(move |x| (x, y))
        })
        .collect()
}
fn selected(s: &Session) -> BTreeSet<(i64, i64)> {
    let selection = s.selection();
    let mut result = BTreeSet::new();
    for y in 0..i64::from(s.document().canvas.1) {
        for x in 0..i64::from(s.document().canvas.0) {
            let covered = match &selection {
                Selection::Mask(m) => m.coverage(x, y) == 255,
                Selection::Rect(r) => r.contains(x, y),
                Selection::Empty => false,
                Selection::Inactive => panic!("active field required"),
            };
            if covered {
                result.insert((x, y));
            }
        }
    }
    if let Selection::Mask(m) = selection {
        for (x, y) in [
            (-1, 0),
            (0, -1),
            (i64::from(s.document().canvas.0), 0),
            (0, i64::from(s.document().canvas.1)),
        ] {
            assert_eq!(m.coverage(x, y), 0);
        }
    }
    result
}
// Independent forward integer orientation, without production coordinate helpers.
fn point(t: T, (x, y): (i64, i64), (w, h): (i64, i64)) -> (i64, i64) {
    match t {
        T::RotateRight => (h - 1 - y, x),
        T::RotateLeft => (y, w - 1 - x),
        T::Rotate180 => (w - 1 - x, h - 1 - y),
        T::FlipHorizontal => (w - 1 - x, y),
        T::FlipVertical => (x, h - 1 - y),
    }
}
#[derive(Clone)]
struct Pixels {
    origin: (i64, i64),
    values: Vec<Vec<Rgba>>,
}
impl Pixels {
    fn from(layer: &Layer) -> Self {
        Self {
            origin: layer.origin,
            values: (0..layer.raster.height())
                .map(|y| {
                    (0..layer.raster.width())
                        .map(|x| layer.raster.get(x.into(), y.into()))
                        .collect()
                })
                .collect(),
        }
    }
    fn extent(&self) -> Rect {
        rect(
            self.origin.0,
            self.origin.1,
            self.values[0].len() as i64,
            self.values.len() as i64,
        )
    }
    fn get(&self, (x, y): (i64, i64)) -> Rgba {
        if self.extent().contains(x, y) {
            self.values[(y - self.origin.1) as usize][(x - self.origin.0) as usize]
        } else {
            Rgba::TRANSPARENT
        }
    }
    fn put(&mut self, (x, y): (i64, i64), color: Rgba) {
        self.values[(y - self.origin.1) as usize][(x - self.origin.0) as usize] = color;
    }
}
#[derive(Clone)]
struct Model {
    canvas: (i64, i64),
    layers: BTreeMap<Uuid, Pixels>,
    selection: BTreeSet<(i64, i64)>,
}
impl Model {
    fn from(s: &Session) -> Self {
        Self {
            canvas: (s.document().canvas.0.into(), s.document().canvas.1.into()),
            layers: s
                .document()
                .layers
                .iter()
                .map(|(id, layer)| (*id, Pixels::from(layer)))
                .collect(),
            selection: ring(),
        }
    }
    fn transform_selection(&mut self, id: Uuid, t: T, clear: Rgba) {
        if self.selection.is_empty() {
            return;
        }
        let minx = self.selection.iter().map(|p| p.0).min().unwrap();
        let maxx = self.selection.iter().map(|p| p.0).max().unwrap() + 1;
        let miny = self.selection.iter().map(|p| p.1).min().unwrap();
        let maxy = self.selection.iter().map(|p| p.1).max().unwrap() + 1;
        let (w, h) = (maxx - minx, maxy - miny);
        let (dw, dh) = if matches!(t, T::RotateRight | T::RotateLeft) {
            (h, w)
        } else {
            (w, h)
        };
        let old = self.layers[&id].clone();
        let bounds = old.extent();
        let (ox, oy) = (bounds.x0.min(minx), bounds.y0.min(miny));
        let (endx, endy) = (bounds.x1.max(minx + dw), bounds.y1.max(miny + dh));
        let mut output = Pixels {
            origin: (ox, oy),
            values: vec![vec![Rgba::TRANSPARENT; (endx - ox) as usize]; (endy - oy) as usize],
        };
        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                output.put(
                    (x, y),
                    if self.selection.contains(&(x, y)) {
                        clear
                    } else {
                        old.get((x, y))
                    },
                );
            }
        }
        let mut mapped = BTreeSet::new();
        for &(x, y) in &self.selection {
            let (dx, dy) = point(t, (x - minx, y - miny), (w, h));
            let p = (minx + dx, miny + dy);
            output.put(p, old.get((x, y)));
            if (0..self.canvas.0).contains(&p.0) && (0..self.canvas.1).contains(&p.1) {
                mapped.insert(p);
            }
        }
        self.layers.insert(id, output);
        self.selection = mapped;
    }
    fn fill(&mut self, id: Uuid, area: Rect, color: Rgba) {
        let layer = self.layers.get_mut(&id).unwrap();
        for &p in &self.selection {
            if area.contains(p.0, p.1) && layer.extent().contains(p.0, p.1) {
                layer.put(p, color);
            }
        }
    }
    fn flood(&mut self, id: Uuid, seed: (i64, i64), color: Rgba) {
        let layer = &self.layers[&id];
        if !self.selection.contains(&seed) || !layer.extent().contains(seed.0, seed.1) {
            return;
        }
        let value = layer.get(seed);
        let mut reached = BTreeSet::from([seed]);
        let mut queue = VecDeque::from([seed]);
        while let Some((x, y)) = queue.pop_front() {
            for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if self.selection.contains(&p)
                    && layer.extent().contains(p.0, p.1)
                    && layer.get(p) == value
                    && reached.insert(p)
                {
                    queue.push_back(p);
                }
            }
        }
        for p in reached {
            self.layers.get_mut(&id).unwrap().put(p, color);
        }
    }
    fn whole(&mut self, t: T) {
        let canvas = self.canvas;
        for layer in self.layers.values_mut() {
            let old = layer.clone();
            let r = old.extent();
            let a = point(t, (r.x0, r.y0), canvas);
            let b = point(t, (r.x1 - 1, r.y1 - 1), canvas);
            let origin = (a.0.min(b.0), a.1.min(b.1));
            let (w, h) = ((a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1);
            *layer = Pixels {
                origin,
                values: vec![vec![Rgba::TRANSPARENT; w as usize]; h as usize],
            };
            for y in r.y0..r.y1 {
                for x in r.x0..r.x1 {
                    layer.put(point(t, (x, y), canvas), old.get((x, y)));
                }
            }
        }
        self.selection = self
            .selection
            .iter()
            .map(|&p| point(t, p, canvas))
            .collect();
        if matches!(t, T::RotateRight | T::RotateLeft) {
            self.canvas = (canvas.1, canvas.0);
        }
    }
    fn assert(&self, s: &Session, original: &Document) {
        assert_eq!(
            s.document().canvas,
            (self.canvas.0 as u32, self.canvas.1 as u32)
        );
        assert_eq!(s.document().document_id, original.document_id);
        assert_eq!(s.document().roots, original.roots);
        assert_eq!(s.document().title, original.title);
        for (id, expected) in &self.layers {
            let actual = &s.document().layers[id];
            let old = &original.layers[id];
            assert_eq!(
                (&actual.name, actual.visible, actual.opacity),
                (&old.name, old.visible, old.opacity)
            );
            assert_eq!(actual.raster.default_pixel(), old.raster.default_pixel());
            assert_eq!(actual.origin, expected.origin);
            assert_eq!(
                (
                    actual.raster.width() as usize,
                    actual.raster.height() as usize
                ),
                (expected.values[0].len(), expected.values.len())
            );
            for (y, row) in expected.values.iter().enumerate() {
                for (x, pixel) in row.iter().enumerate() {
                    assert_eq!(
                        actual.raster.get(x as i64, y as i64),
                        *pixel,
                        "layer={id} at{x},{y}"
                    );
                }
            }
        }
        assert_eq!(selected(s), self.selection);
    }
}
fn transform(id: Uuid, t: T, clear: Rgba) -> Op {
    Op::TransformSelection {
        layer: LayerRef::Id(id),
        transform: t,
        clear,
    }
}
fn fill(id: Uuid, area: Rect, color: Rgba) -> Op {
    Op::FillRect {
        layer: LayerRef::Id(id),
        rect: area,
        color,
        mode: Mode::Replace,
    }
}
#[test]
fn five_masked_orientations_match_independent_tight_cut_paste_and_exact_history() {
    for t in ALL {
        for origin in [(-2, -1), (3, 2), (7, 4)] {
            for clear in [Rgba::TRANSPARENT, Rgba([255; 4])] {
                let (mut s, id) = setup(origin);
                apply(&mut s, vec![polygon()]);
                assert_eq!(selected(&s), ring());
                let before = s.snapshot();
                let selection = s.selection();
                let mut expected = Model::from(&s);
                expected.transform_selection(id, t, clear);
                apply(&mut s, vec![transform(id, t, clear)]);
                expected.assert(&s, &before);
                let after = s.snapshot();
                let after_selection = s.selection();
                s.undo().unwrap().unwrap();
                assert!(s.document().same_semantics(&before));
                assert_eq!(s.selection(), selection);
                s.redo().unwrap().unwrap();
                assert!(s.document().same_semantics(&after));
                assert_eq!(s.selection(), after_selection);
            }
        }
    }
}
#[test]
fn compound_masked_transforms_paint_flood_whole_canvas_and_replay_use_correct_operation_geometry() {
    let (mut s, id) = setup((3, 2));
    let original = s.snapshot();
    let mut expected = Model::from(&s);
    let gray = Rgba([70, 80, 90, 0]);
    let green = Rgba([0, 200, 0, 255]);
    let blue = Rgba([0, 0, 255, 128]);
    let area = rect(4, 3, 2, 2);
    expected.fill(id, area, gray);
    expected.transform_selection(id, T::RotateRight, Rgba([255; 4]));
    let column = rect(4, 3, 1, 3);
    expected.fill(id, column, green);
    let seed = *expected.selection.iter().next().unwrap();
    expected.flood(id, seed, blue);
    expected.transform_selection(id, T::FlipHorizontal, Rgba::TRANSPARENT);
    expected.whole(T::RotateLeft);
    expected.transform_selection(id, T::Rotate180, Rgba([255; 4]));
    let ops = vec![
        polygon(),
        fill(id, area, gray),
        transform(id, T::RotateRight, Rgba([255; 4])),
        fill(id, column, green),
        Op::FloodFill {
            layer: LayerRef::Id(id),
            at: seed,
            color: blue,
            mode: Mode::Replace,
            tolerance: 0,
        },
        transform(id, T::FlipHorizontal, Rgba::TRANSPARENT),
        Op::CanvasTransform {
            transform: T::RotateLeft,
        },
        transform(id, T::Rotate180, Rgba([255; 4])),
    ];
    apply(&mut s, ops);
    expected.assert(&s, &original);
    let after = s.snapshot();
    let selection = s.selection();
    s.undo().unwrap().unwrap();
    assert!(s.document().same_semantics(&original));
    assert_eq!(s.selection(), Selection::Inactive);
    s.redo().unwrap().unwrap();
    assert!(s.document().same_semantics(&after));
    assert_eq!(s.selection(), selection);
}
fn cancelled() -> Result<()> {
    Err(Error::new(
        ErrorCode::OperationCancelled,
        "cancel masked engine transform",
    ))
}
#[test]
fn cancellations_denials_and_late_publication_keep_owner_history_and_release_private_mask_geometry()
{
    let (mut s, id) = setup((3, 2));
    apply(&mut s, vec![polygon()]);
    let before = s.snapshot();
    let selection = s.selection();
    let ticket = s.ticket();
    let history = s.history_view();
    let ops = vec![
        transform(id, T::RotateRight, Rgba([255; 4])),
        Op::CanvasTransform {
            transform: T::FlipVertical,
        },
        transform(id, T::Rotate180, Rgba::TRANSPARENT),
    ];
    let b = batch(&s, ops);
    let masks = SharedBudget::new(8 << 20);
    let metadata = SharedBudget::new(8 << 20);
    let decoded = SharedBudget::new(8 << 20);
    let admission = Admission::with_residency(1_000_000, decoded.clone())
        .with_selection_budget(masks.clone())
        .with_paint_metadata_budget(metadata.clone());
    let count = Cell::new(0);
    drop(
        s.preparation()
            .unwrap()
            .apply_with_admission(&b, admission.clone(), || {
                count.set(count.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    assert_eq!(
        (
            masks.usage().reserved,
            metadata.usage().reserved,
            decoded.usage().reserved
        ),
        (0, 0, 0)
    );
    // Exercise checkpoints across geometry construction, logical compilation and decoded replay.
    for at in 1..=count.get() {
        let n = Cell::new(0);
        let result = s
            .preparation()
            .unwrap()
            .apply_with_admission(&b, admission.clone(), || {
                n.set(n.get() + 1);
                if n.get() == at { cancelled() } else { Ok(()) }
            });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint {at}/{}",
            count.get()
        );
        assert_eq!(
            (
                masks.usage().reserved,
                metadata.usage().reserved,
                decoded.usage().reserved
            ),
            (0, 0, 0)
        );
    }
    for denied in [
        Admission::new(0).with_selection_budget(masks.clone()),
        Admission::new(1_000_000).with_selection_budget(SharedBudget::new(0)),
        Admission::new(1_000_000).with_paint_metadata_budget(SharedBudget::new(0)),
    ] {
        assert_eq!(
            s.preparation()
                .unwrap()
                .apply_with_admission(&b, denied, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    // Admit the first transformed field, then deny a later field while its
    // earlier compiled geometry remains live. No prefix may reach the owner.
    let first_budget = SharedBudget::new(8 << 20);
    let first = batch(&s, vec![transform(id, T::RotateRight, Rgba([255; 4]))]);
    drop(
        s.preparation()
            .unwrap()
            .apply_with_admission(
                &first,
                Admission::new(1_000_000).with_selection_budget(first_budget.clone()),
                || Ok(()),
            )
            .unwrap(),
    );
    let prefix_budget = SharedBudget::new(first_budget.usage().peak);
    let late = s
        .preparation()
        .unwrap()
        .apply_with_admission(
            &b,
            Admission::new(1_000_000).with_selection_budget(prefix_budget.clone()),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(late.code, ErrorCode::ResourceLimit);
    assert_eq!(late.operation_index, Some(1));
    assert_eq!(prefix_budget.usage().reserved, 0);
    let prepared = s
        .preparation()
        .unwrap()
        .apply_with_admission(&b, admission.clone(), || Ok(()))
        .unwrap();
    assert_eq!(
        s.publish_prepared(prepared, cancelled).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), history);
    assert_eq!(s.selection(), selection);
    assert!(Arc::ptr_eq(&s.snapshot(), &before));
    let prepared = s
        .preparation()
        .unwrap()
        .apply_with_admission(&b, admission, || Ok(()))
        .unwrap();
    apply(&mut s, vec![Op::SelectionClear]);
    let current = s.ticket();
    let current_history = s.history_view();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.ticket(), current);
    assert_eq!(s.history_view(), current_history);
    assert_eq!(s.selection(), Selection::Inactive);
    assert!(s.document().same_semantics(&before));
    assert_eq!(
        (
            masks.usage().reserved,
            metadata.usage().reserved,
            decoded.usage().reserved
        ),
        (0, 0, 0)
    );
}
