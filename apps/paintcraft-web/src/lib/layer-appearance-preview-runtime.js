// Separate Source7 Worker entry. Existing Source2..6 runtime stays byte-exact.
import initialize, {
	LayerAppearanceRenderWorker,
	layer_appearance_capabilities,
	layer_appearance_protocol_version,
	layer_appearance_ready,
	linear_memory_ceiling,
} from '../../render-worker-pkg/paintcraft_web_render_worker.js';
import { createLayerAppearanceRuntime } from './layer-appearance-preview-dispatch.js';

const runtime = createLayerAppearanceRuntime({
	initialize,
	api: {
		LayerAppearanceRenderWorker,
		layer_appearance_capabilities,
		layer_appearance_protocol_version,
		layer_appearance_ready,
		linear_memory_ceiling,
	},
	endpoint: self,
	Channel: MessageChannel,
});
self.onmessage = runtime.receive;
