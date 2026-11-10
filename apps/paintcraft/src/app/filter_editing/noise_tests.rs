//! Actual native Studio Noise controls and asynchronous appearance rendering.
use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};

const RAW_NOISE: [u8; 8] = [64, 16, 128, 255, 64, 16, 128, 255];

fn noise_app() -> App {
    let mut a = app(false);
    a.editor = Editor::with_document(
        paintcraft_document::new_document(
            2,
            1,
            Some(Rgba([64, 16, 128, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap(),
        None,
    );
    a.initialize_editing(a.context.clone());
    a.renderer = Some(
        Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap(),
    );
    a
}

fn open_noise(h: &mut Harness<'_, App>, distribution: NoiseDistribution) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Add Noise").click();
    h.run();
    h.get_by_role_and_label(egui::Role::Slider, "Noise amount (%)")
        .set_value_accesskit(100.0);
    h.run();
    h.get_by_role_and_label(egui::Role::Slider, "Noise seed")
        .set_value_accesskit(65535.0);
    h.run();
    h.get_by_label("Monochrome noise").click();
    h.run();
    if distribution == NoiseDistribution::Gaussian {
        h.get_by_role_and_label(egui::Role::ComboBox, "Noise distribution")
            .click();
        h.run();
        h.get_by_label("Gaussian noise").click();
        h.run();
    }
}

#[test]
fn noise_modal_private_preview_cancel_apply_project_and_undo_redo() {
    // Existing independent native background/ordinary-output literal goldens.
    for (distribution, expected) in [
        (
            NoiseDistribution::Uniform,
            [255, 211, 0, 255, 51, 0, 97, 255],
        ),
        (
            NoiseDistribution::Gaussian,
            [47, 139, 0, 255, 0, 4, 46, 255],
        ),
    ] {
        let mut h = harness(noise_app());
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        let history = h.state().editor.history_view();
        open_noise(&mut h, distribution);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        assert_eq!(h.state().editor.undo_depth(), 0);
        assert!(!h.state().editor.published_is_dirty());
        assert!(h.state().editor.export_snapshot().is_err());
        let preview_key = h.state().presentation_key();
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW_NOISE);
        assert!(!h.state().accept_render_key(preview_key));
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        open_noise(&mut h, distribution);
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert_eq!(h.state().editor.undo_depth(), 1);
        let doc = h.state().editor.document();
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(
            layer.filters.as_slice()[0].descriptor(),
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap())
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 6);
        let path =
            std::env::temp_dir().join(format!("paintcraft-native-noise-{}.pcnd", Uuid::new_v4()));
        struct Remove(std::path::PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _remove = Remove(path.clone());
        std::fs::write(&path, bytes).unwrap();
        assert!(Editor::open(&path).unwrap().document().same_semantics(doc));
        h.state_mut().editor.request(9901, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW_NOISE);
        assert!(h.state().editor.document().same_semantics(&source));
        assert_eq!(h.state().editor.undo_depth(), 0);
        h.state_mut().editor.request(9902, Action::Redo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
    }
}
