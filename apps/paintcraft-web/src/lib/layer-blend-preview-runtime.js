// Dedicated Source8 Worker entry; legacy Worker entry modules remain unchanged.
import initialize, {
	LayerBlendRenderWorker,
	layer_blend_runtime_fact,
	layer_blend_control_fact,
	layer_blend_profile_word,
	layer_blend_live_instances,
} from '../../render-worker-pkg/paintcraft_web_render_worker.js';
import { createLayerBlendRuntime } from './layer-blend-preview-dispatch.js';
const runtime = createLayerBlendRuntime({
	initialize,
	api: {
		LayerBlendRenderWorker,
		layer_blend_runtime_fact,
		layer_blend_control_fact,
		layer_blend_profile_word,
		layer_blend_live_instances,
	},
	endpoint: self,
});
self.onmessage = (event) => {
	runtime.receive(event).catch(() => {});
};
