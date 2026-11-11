//! Explicit required-device exercise of the native render-job fallback, not a direct CPU oracle.
use super::*;
#[test]
#[ignore = "Explicit required-adapter native Color Overlay GPU-to-CPU fallback qualification"]
fn required_gpu_full_and_scaled_style_preview_fallback_preserves_private_owner() {
    for level in [0, 1] {
        let (target, gpu) = gpu_display::tests::target()
            .expect("This explicit qualification requires a real Wgpu adapter; do not report a skipped pass");
        eprintln!(
            "Color Overlay fallback adapter: {:?}",
            target.adapter_info()
        );
        let mut a = app();
        a.gpu_completion = Some(paintcraft_render::gpu::CompletionDriver::new(&gpu).unwrap());
        a.gpu = Some(gpu);
        a.gpu_target = Some(target);
        // Exercise the actual scaled GPU entry too, rather than the intentional
        // reduced-display CPU branch when scaled GPU display is disabled.
        a.gpu_scaled_display = true;
        a.render_level = level;
        let source = a.editor.document_snapshot();
        let ticket = a.editor.render_key();
        let history = a.editor.history_view();
        let dirty = a.editor.published_is_dirty();
        let mut h = harness(a);
        h.run();
        assert!(h.state().gpu.is_some());
        open(&mut h);
        let ctx = h.ctx.clone();
        frame(h.state_mut(), &ctx);
        let expected: &[u8] = if level == 0 { &RED } else { &[255, 0, 0, 255] };
        assert_eq!(h.state().rendered, expected);
        assert!(
            h.state().gpu_texture.is_none(),
            "unsupported styled GPU output must never install"
        );
        assert!(
            h.state().texture.is_some(),
            "actual fallback installs the CPU texture"
        );
        let prefix = if level == 0 {
            "Using CPU rendering:"
        } else {
            "Scaled display used CPU:"
        };
        assert!(
            h.state().editor.status().starts_with(prefix),
            "actual GPU refusal did not reach fallback: {}",
            h.state().editor.status()
        );
        assert!(h.state().editor.status().contains("Layer styles"));
        assert_eq!(
            h.state().gpu.is_some(),
            level != 0,
            "existing full/scaled backend retirement policy is preserved"
        );
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.render_key(), ticket);
        assert_eq!(h.state().editor.history_view(), history);
        assert_eq!(h.state().editor.published_is_dirty(), dirty);
        h.get_by_label("Cancel").click();
        h.run();
        // Returning to full-size raw source does not create a publication or
        // reuse the stale style result. CPU/GPU output choice remains existing policy.
        h.state_mut().render_level = 0;
        // Scaled refusal intentionally leaves the genuine GPU available. The
        // committed raw canvas may use it; read the authoritative raw input here.
        assert!(h.state().style_editing.is_none());
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        assert_eq!(
            source.layers[&source.roots[0]].raster.get(0, 0),
            Rgba([10, 20, 30, 255])
        );
        assert_eq!(
            source.layers[&source.roots[0]].raster.get(1, 0),
            Rgba([100, 150, 200, 255])
        );
    }
}
