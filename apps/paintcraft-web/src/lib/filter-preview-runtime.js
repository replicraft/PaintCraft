// Persistent visual Worker; separate from the one-shot format codec instance.
import initialize, {
	linear_memory_ceiling,
	protocol_version,
	RenderWorker,
} from '../../render-worker-pkg/paintcraft_web_render_worker.js';
import {
	canonical,
	fail,
	LINEAR_MEMORY_BYTES,
	SOURCE_BYTES_MAX,
	validatePlan,
} from './filter-preview-protocol.js';

let service = null,
	current = null,
	desired = null,
	scheduled = false,
	opening = false;
function post(message, transfer = []) {
	self.postMessage(message, transfer);
}
function failure(id, error) {
	let parsed;
	try {
		if (typeof error === 'string' && error.length <= 4096)
			parsed = JSON.parse(error);
	} catch {
		/* typed JS or trap */
	}
	post({
		kind: 'error',
		id,
		completed:
			typeof parsed?.code === 'string' &&
			[
				'input.schema',
				'policy.unknown',
				'reference.unknown_alias',
				'reference.unknown_id',
				'target.invalid',
				'capability.unsupported',
				'conflict.stale_base',
				'resource.limit',
				'operation.cancelled',
				'codec.failed',
				'storage.failed',
				'invariant.violation',
			].includes(parsed.code),
		code:
			typeof parsed?.code === 'string'
				? parsed.code.slice(0, 64)
				: typeof error?.code === 'string'
					? error.code.slice(0, 64)
					: 'CodecFailed',
		message:
			typeof parsed?.message === 'string'
				? parsed.message.slice(0, 4096)
				: 'Render Worker failed',
	});
}
function retired(request) {
	if (request)
		post({
			kind: 'retired',
			id: request.id,
			generation: request.request.generation,
		});
}
function retire() {
	service?.retire();
	retired(current);
	current = null;
	retired(desired);
	desired = null;
}
const steps = new MessageChannel();
steps.port1.onmessage = pump;
function schedule() {
	if (!scheduled) {
		scheduled = true;
		steps.port2.postMessage(0);
	}
}
function pump() {
	scheduled = false;
	try {
		if (desired) {
			if (current) {
				service.retire();
				retired(current);
				current = null;
			}
			current = desired;
			desired = null;
			current.binding = JSON.parse(
				service.prepare(canonical(current.request)),
			);
			if (!current.request.render) {
				post({ ...current.binding, id: current.id });
				current = null;
				return;
			}
		}
		if (!current) return;
		if (service.step()) {
			const pixels = service.frame();
			post({ ...current.binding, id: current.id, pixels }, [
				pixels.buffer,
			]);
			current = null;
		} else schedule();
	} catch (error) {
		const id = current?.id;
		service?.retire();
		current = null;
		failure(id, error);
	}
}
self.onmessage = async ({ data }) => {
	let ownsOpening = false;
	try {
		if (data.kind === 'open') {
			if (service || opening)
				throw fail('InputSchema', 'Source already open');
			opening = true;
			ownsOpening = true;
			if (
				!(data.source instanceof ArrayBuffer) ||
				data.source.byteLength < 1 ||
				data.source.byteLength > SOURCE_BYTES_MAX
			)
				throw fail('ResourceLimit', 'Filter source extent');
			validatePlan(data.plan);
			const encoded = canonical(data.plan);
			const wasm = await initialize();
			if (
				![2, 3, 4].includes(protocol_version()) ||
				(data.plan.protocol === 3 &&
					![3, 4].includes(protocol_version())) ||
				(data.plan.protocol === 4 && protocol_version() !== 4) ||
				linear_memory_ceiling() !== LINEAR_MEMORY_BYTES ||
				wasm.memory.buffer.byteLength > LINEAR_MEMORY_BYTES
			)
				throw fail('CodecFailed', 'Render Worker contract mismatch');
			service = new RenderWorker(new Uint8Array(data.source), encoded);
			opening = false;
			post({
				kind: 'ready',
				id: data.id,
				binding: JSON.parse(service.binding()),
			});
			return;
		}
		if (!service) throw fail('InputSchema', 'Source not open');
		if (data.kind === 'update') {
			retired(desired);
			desired = data;
			schedule();
		} else if (data.kind === 'cancel') {
			const through = BigInt(data.generation);
			if (current && BigInt(current.request.generation) <= through) {
				service.retire();
				retired(current);
				current = null;
			}
			if (desired && BigInt(desired.request.generation) <= through) {
				retired(desired);
				desired = null;
			}
			if (!current && !desired) service.retire();
			post({ kind: 'retired', id: data.id, generation: data.generation });
		} else if (data.kind === 'close') {
			retire();
			service.free();
			service = null;
			post({ kind: 'closed', id: data.id });
		} else throw fail('InputSchema', 'Unknown render Worker operation');
	} catch (error) {
		if (ownsOpening) opening = false;
		failure(data?.id, error);
	}
};
