// Persistent visual Worker; separate from the one-shot format codec instance.
import initialize, {
	flat_copy_limits,
	flat_copy_protocol_version,
	linear_memory_ceiling,
	NoiseRenderWorker,
	prepared_capabilities,
	prepared_protocol_version,
	protocol_version,
	RenderWorker,
	scoped_capabilities,
	scoped_control_quote,
	scoped_protocol_version,
	scoped_qualification_policy,
} from '../../render-worker-pkg/paintcraft_web_render_worker.js';
import { validateCopyLimits, validateCopySpec } from './filter-preview-copy.js';
import {
	cancelGeneration6,
	validateRuntimeRequest6,
} from './filter-preview-noise-protocol.js';
import {
	BEGIN_BYTES_MAX,
	CAPABILITY_BYTES_MAX,
	encodeProgram,
	OPEN_BYTES_MAX,
	validatePairs,
} from './filter-preview-program-protocol.js';
import {
	canonical,
	fail,
	LINEAR_MEMORY_BYTES,
	SOURCE_BYTES_MAX,
	validatePlan,
	validateRequest,
} from './filter-preview-protocol.js';

let sourceProtocol = 2;
let pendingCopy = null,
	copyFinalizing = null,
	copyFinalizingRequest = null;
let sourcePlan = null,
	lastQueued = -1n,
	acceptedGeneration = -1n;
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
	pendingCopy = null;
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
			if (sourceProtocol === 6)
				acceptedGeneration = BigInt(current.request.generation);
			current.binding = JSON.parse(
				service.prepare(
					[5, 6].includes(sourceProtocol)
						? encodeProgram(current.request, BEGIN_BYTES_MAX)
						: canonical(current.request),
				),
			);
			if (!current.request.render) {
				post({ ...current.binding, id: current.id });
				current = null;
				return;
			}
		}
		if (!current) return;
		if (service.step()) {
			if (current.copy) {
				const bytes = service.encodedFrame(
					new Uint8Array(current.mask),
				);
				const output = current.copy;
				const binding = current.binding;
				const id = current.id;
				current.mask = null;
				current = null;
				const bytesSha256 = awaitCopySha(bytes);
				copyFinalizing = bytesSha256;
				copyFinalizingRequest = { id, request: output.request };
				// Crypto is asynchronous; no next action is accepted while this
				// bounded final envelope is pending. Close/cancel invalidates it.
				bytesSha256
					.then((digest) => {
						if (!service || pendingCopy !== output) return;
						post(
							{
								kind: 'encoded-copy',
								id,
								outputVersion: 1,
								outputSha256: output.outputSha256,
								bytesSha256: digest,
								mime: output.mime,
								binding,
								bytes,
							},
							[bytes.buffer],
						);
						pendingCopy = null;
					})
					.catch((error) => failure(id, error))
					.finally(() => {
						copyFinalizing = null;
						copyFinalizingRequest = null;
					});
				return;
			}
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
async function awaitCopySha(bytes) {
	return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
		.map((v) => v.toString(16).padStart(2, '0'))
		.join('');
}
self.onmessage = async ({ data }) => {
	let ownsOpening = false;
	try {
		if (data.kind === 'hello' && data.registryContract === 2) {
			if (data.protocol !== 6)
				throw fail('CapabilityUnsupported', 'Source6 HELLO protocol');
			const control = encodeProgram(
				{
					protocol: 6,
					registryContract: 2,
					pairs: data.pairs,
					workload: data.workload,
				},
				CAPABILITY_BYTES_MAX,
			);
			const wasm = await initialize();
			if (
				scoped_protocol_version() !== 6 ||
				linear_memory_ceiling() !== LINEAR_MEMORY_BYTES ||
				wasm.memory.buffer.byteLength > LINEAR_MEMORY_BYTES
			)
				throw fail('CodecFailed', 'Source6 artifact contract');
			post({
				...JSON.parse(scoped_capabilities(control)),
				id: data.id,
				runtimeFacts: {
					controlQuote: JSON.parse(scoped_control_quote()),
					slicePolicy: JSON.parse(scoped_qualification_policy()),
				},
			});
			return;
		}
		if (data.kind === 'hello') {
			if (data.registryContract !== 1)
				throw fail('InputSchema', 'Prepared registry contract');
			validatePairs(data.pairs);
			const control = encodeProgram(
				{ registryContract: 1, pairs: data.pairs },
				CAPABILITY_BYTES_MAX,
			);
			const wasm = await initialize();
			if (
				prepared_protocol_version() !== 5 ||
				linear_memory_ceiling() !== LINEAR_MEMORY_BYTES ||
				wasm.memory.buffer.byteLength > LINEAR_MEMORY_BYTES
			)
				throw fail('CodecFailed', 'Prepared Worker contract mismatch');
			post({
				...JSON.parse(prepared_capabilities(control)),
				id: data.id,
			});
			return;
		}
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
			const scoped = data.plan.protocol === 6;
			const generic = data.plan.protocol === 5;
			const encoded =
				generic || scoped
					? encodeProgram(data.plan, OPEN_BYTES_MAX)
					: canonical(data.plan);
			const wasm = await initialize();
			if (
				(scoped
					? scoped_protocol_version() !== 6
					: generic
						? prepared_protocol_version() !== 5
						: ![2, 3, 4].includes(protocol_version())) ||
				(data.plan.protocol === 3 &&
					![3, 4].includes(protocol_version())) ||
				(data.plan.protocol === 4 && protocol_version() !== 4) ||
				linear_memory_ceiling() !== LINEAR_MEMORY_BYTES ||
				wasm.memory.buffer.byteLength > LINEAR_MEMORY_BYTES
			)
				throw fail('CodecFailed', 'Render Worker contract mismatch');
			service = scoped
				? NoiseRenderWorker.openQualified(
						new Uint8Array(data.source),
						encoded,
					)
				: generic
					? RenderWorker.openPrograms(
							new Uint8Array(data.source),
							encoded,
						)
					: new RenderWorker(new Uint8Array(data.source), encoded);
			sourceProtocol = data.plan.protocol ?? 2;
			sourcePlan = data.plan;
			lastQueued = -1n;
			acceptedGeneration = -1n;
			opening = false;
			post({
				kind: 'ready',
				id: data.id,
				binding: JSON.parse(service.binding()),
			});
			return;
		}
		if (!service) throw fail('InputSchema', 'Source not open');
		if (data.kind === 'copy-prepare') {
			if (
				sourceProtocol !== 6 ||
				current ||
				desired ||
				pendingCopy ||
				copyFinalizing
			)
				throw fail('ResourceLimit', 'Flat output Worker busy');
			validateRuntimeRequest6(
				data.request,
				sourcePlan,
				validateRequest,
				canonical,
			);
			const control = validateCopySpec(
				data.output,
				data.request,
				sourcePlan,
			);
			if (flat_copy_protocol_version() !== 1)
				throw fail(
					'CapabilityUnsupported',
					'Flat output artifact version',
				);
			const limits = JSON.parse(flat_copy_limits());
			validateCopyLimits(limits);
			const generation = BigInt(data.request.generation);
			if (generation <= lastQueued)
				throw fail('ConflictStaleBase', 'Flat output generation');
			const admitted = JSON.parse(
				service.prepareCopy(
					encodeProgram(data.request, BEGIN_BYTES_MAX),
					control,
				),
			);
			acceptedGeneration = generation;
			lastQueued = generation;
			pendingCopy = {
				request: data.request,
				binding: admitted.binding,
				outputSha256: admitted.outputSha256,
				maskLength: data.output.maskLength,
				mime: {
					png: 'image/png',
					jpeg: 'image/jpeg',
					bmp: 'image/bmp',
				}[data.output.format],
			};
			post({ ...admitted, id: data.id, limits });
			return;
		}
		if (data.kind === 'copy-input') {
			if (
				!pendingCopy ||
				current ||
				desired ||
				data.outputSha256 !== pendingCopy.outputSha256 ||
				!(data.mask instanceof ArrayBuffer) ||
				data.mask.byteLength !== pendingCopy.maskLength
			)
				throw fail('ConflictStaleBase', 'Flat output selection input');
			current = {
				id: data.id,
				request: pendingCopy.request,
				binding: pendingCopy.binding,
				copy: pendingCopy,
				mask: data.mask,
			};
			schedule();
			return;
		}
		if (data.kind === 'update') {
			if (sourceProtocol === 6) {
				if (pendingCopy || copyFinalizing)
					throw fail(
						'ResourceLimit',
						'Flat output owns current generation',
					);
				validateRuntimeRequest6(
					data.request,
					sourcePlan,
					validateRequest,
					canonical,
				);
				const generation = BigInt(data.request.generation);
				if (generation <= lastQueued)
					throw fail(
						'ConflictStaleBase',
						'Source6 generation did not advance',
					);
				lastQueued = generation;
			}
			retired(desired);
			desired = data;
			schedule();
		} else if (data.kind === 'cancel') {
			const through =
				sourceProtocol === 6
					? cancelGeneration6(data.generation)
					: BigInt(data.generation);
			if (sourceProtocol === 6 && through > lastQueued)
				lastQueued = through;
			if (
				copyFinalizingRequest &&
				BigInt(copyFinalizingRequest.request.generation) <= through
			) {
				retired(copyFinalizingRequest);
				copyFinalizingRequest = null;
			}
			if (
				pendingCopy &&
				BigInt(pendingCopy.request.generation) <= through
			)
				pendingCopy = null;
			if (current && BigInt(current.request.generation) <= through) {
				service.retire();
				retired(current);
				current = null;
			}
			if (desired && BigInt(desired.request.generation) <= through) {
				retired(desired);
				desired = null;
			}
			if (
				!current &&
				!desired &&
				(sourceProtocol !== 6 ||
					(acceptedGeneration >= 0n && acceptedGeneration <= through))
			)
				service.retire();
			post({ kind: 'retired', id: data.id, generation: data.generation });
		} else if (data.kind === 'close') {
			if (copyFinalizing) await copyFinalizing.catch(() => {});
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
