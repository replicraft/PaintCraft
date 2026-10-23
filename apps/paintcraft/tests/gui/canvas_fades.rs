use super::*;
use eframe::egui::{Color32, Rect, TextureId, epaint::Shape};

#[derive(Clone, Copy, Debug)]
enum Frontend {
    Classic,
    RuledClassic,
    Studio,
}
struct Scene {
    frontend: Frontend,
    clip: Option<Rect>,
    origin: Pos2,
}

fn scene(frontend: Frontend, dark: bool, ppp: f32) -> Harness<'static, Scene> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(400.0, 300.0))
        .with_pixels_per_point(ppp)
        .build_ui_state(
            move |ui, state: &mut Scene| {
                ui.ctx().set_visuals(if dark {
                    eframe::egui::Visuals::dark()
                } else {
                    eframe::egui::Visuals::light()
                });
                let fade = ui.spacing().scroll.fade;
                let id = eframe::egui::Id::unique("canvas-fade-fixture");
                let size = Vec2::splat(2000.0);
                let paint = |_: &mut eframe::egui::Ui,
                             response: eframe::egui::Response,
                             painter: eframe::egui::Painter| {
                    painter.rect_filled(response.rect, 0, Color32::WHITE);
                    (
                        painter.clip_rect().intersect(response.rect),
                        response.rect.min,
                    )
                };
                let (clip, origin) = match state.frontend {
                    Frontend::Classic => {
                        paintcraft_classic::canvas::canvas(ui, id, size, 0.0, paint)
                    }
                    Frontend::RuledClassic => paintcraft_classic::canvas::ruled_canvas(
                        ui,
                        id,
                        size,
                        (2000, 2000),
                        0.0,
                        paint,
                    ),
                    Frontend::Studio => paintcraft_studio::canvas::canvas(ui, id, size, paint),
                };
                state.clip = Some(clip);
                state.origin = origin;
                assert_eq!(
                    ui.spacing().scroll.fade,
                    fade,
                    "canvas styling must not change the caller's scroll policy"
                );
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("test: repeated canvas-fade layout");
                }
            },
            Scene {
                frontend,
                clip: None,
                origin: Pos2::ZERO,
            },
        );
    h.run();
    h
}

fn pan(h: &mut Harness<Scene>) {
    let origin = h.state().origin;
    let point = h.state().clip.unwrap().center();
    h.input_mut().events.extend([
        Event::PointerMoved(point),
        Event::MouseWheel {
            unit: eframe::egui::MouseWheelUnit::Point,
            delta: Vec2::new(-200.0, -160.0),
            phase: eframe::egui::TouchPhase::Move,
            source: eframe::egui::MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run_steps(12);
    assert!(
        h.state().origin.x < origin.x && h.state().origin.y < origin.y,
        "wheel input must actually pan both axes"
    );
    // The screenshot harness draws its pointer; keep it outside sampled pixels.
    h.event(Event::PointerMoved(Pos2::ZERO));
    h.run_steps(2);
}

#[test]
fn canvas_chrome_does_not_overlay_scroll_edge_gradients_or_change_caller_style() {
    for frontend in [Frontend::Classic, Frontend::RuledClassic, Frontend::Studio] {
        for (ppp, dark) in [(1.0, false), (2.0, true)] {
            let mut h = scene(frontend, dark, ppp);
            for panned in [false, true] {
                if panned {
                    pan(&mut h);
                }
                let clip = h.state().clip.unwrap();
                assert!(!h.output().shapes.iter().any(|shape| matches!(&shape.shape, Shape::Mesh(mesh)
                    if mesh.texture_id == TextureId::default() && clip.intersects(mesh.calc_bounds())
                        && mesh.vertices.iter().any(|vertex| vertex.color.a() > 0)
                        && mesh.vertices.iter().any(|vertex| vertex.color.a() == 0))),
                    "{frontend:?}, scale {ppp}, panned={panned}: canvas has an edge fade");
            }
        }
    }
}

#[test]
#[ignore = "requires GPU; checks actual canvas edge pixels in both frontends"]
fn rendered_canvas_pixels_remain_white_at_clipped_edges_before_and_after_pan() {
    for frontend in [Frontend::Classic, Frontend::RuledClassic, Frontend::Studio] {
        for (ppp, dark) in [(1.0, false), (2.0, true)] {
            let mut h = scene(frontend, dark, ppp);
            for panned in [false, true] {
                if panned {
                    pan(&mut h);
                }
                let clip = h.state().clip.unwrap();
                let center = clip.center();
                let points = [
                    center,
                    Pos2::new(clip.right() - 15.0, center.y),
                    Pos2::new(center.x, clip.bottom() - 15.0),
                    Pos2::new(clip.left() + 15.0, center.y),
                    Pos2::new(center.x, clip.top() + 15.0),
                ];
                let image = h.render().unwrap();
                for point in points {
                    let pixel = image
                        .get_pixel(
                            (point.x * ppp).round() as u32,
                            (point.y * ppp).round() as u32,
                        )
                        .0;
                    assert_eq!(
                        pixel, [255; 4],
                        "{frontend:?}, scale {ppp}, panned={panned}, point={point:?}"
                    );
                }
            }
        }
    }
}
