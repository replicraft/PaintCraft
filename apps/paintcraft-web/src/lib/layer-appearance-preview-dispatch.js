// Source7 one-slot dispatcher. No process-default pool, grant or raw-source cache.
import {
	equal7,
	freeze7,
	numericalDigests7,
	numericalEntry7,
	numericalFacts7,
	sha7,
	snapshot7,
	validateBegin7,
	validateCapabilities7,
	validateContext7,
	validateCopyAdmitted7,
	validateHello7,
	validatePlan7,
	validateReady7,
	verifyBegin7,
	verifyBinding7,
	verifyPlan7,
} from './filter-preview-appearance-protocol.js';
import {
	BEGIN_BYTES_MAX,
	CAPABILITY_BYTES_MAX,
	encodeProgram,
	OPEN_BYTES_MAX,
	unionPairs,
} from './filter-preview-program-protocol.js';
import {
	fail,
	LINEAR_MEMORY_BYTES,
	SOURCE_BYTES_MAX,
	sha,
} from './filter-preview-protocol.js';
import {
	error7,
	generation7,
	json7,
	message7,
	pngExtent7,
	selection7,
	sourceBuffer7,
	visualSpec7,
} from './layer-appearance-preview-payload.js';

export function createLayerAppearanceRuntime({
	initialize,
	api,
	endpoint,
	Channel,
	digestEncoded = sha,
}) {
	let phase = 'new',
		closing = false,
		closed = false,
		poisoned = false;
	let ready = null,
		hello = null,
		entries = null,
		plan = null,
		service = null;
	let lastId = 0,
		lastQueued = -1n,
		retiredThrough = -1n,
		rustGeneration = null;
	let desired = null,
		validating = null,
		current = null,
		copyPending = null,
		finishing = null;
	let scheduled = false,
		pumping = false,
		loading = null,
		closingTask = null;
	const rustOrigin = Symbol('actual Source7 FFI exception');
	class FfiFailure extends Error {
		constructor(error) {
			super('Source7 FFI exception', { cause: error });
			this[rustOrigin] = true;
		}
	}
	function ffi(call) {
		try {
			return call();
		} catch (error) {
			throw new FfiFailure(error);
		}
	}
	function detail(error) {
		return error?.[rustOrigin] === true ? error.cause : error;
	}
	const tasks = new Set();
	const steps = new Channel();
	function post(value, transfer = []) {
		endpoint.postMessage(value, transfer);
	}
	function track(task) {
		tasks.add(task);
		task.finally(() => tasks.delete(task)).catch(() => {});
		return task;
	}
	function live(ticket) {
		return (
			!closed &&
			!closing &&
			!ticket.cancelled &&
			ticket.generation > retiredThrough
		);
	}
	function retired(ticket) {
		if (!ticket || ticket.responded) return;
		ticket.cancelled = true;
		ticket.responded = true;
		post({
			kind: 'retired',
			id: ticket.id,
			generation: ticket.request.generation,
		});
	}
	function failure(id, error, completed = false) {
		post({
			kind: 'error',
			id,
			...error7(detail(error), completed && error?.[rustOrigin] === true),
		});
	}
	async function load() {
		loading ??= initialize();
		const wasm = await loading;
		if (
			api.layer_appearance_protocol_version() !== 7 ||
			api.linear_memory_ceiling() !== LINEAR_MEMORY_BYTES ||
			!(wasm?.memory?.buffer instanceof ArrayBuffer) ||
			wasm.memory.buffer.byteLength > LINEAR_MEMORY_BYTES
		)
			throw fail('CodecFailed', 'Source7 genuine artifact contract');
		return wasm;
	}
	function schedule() {
		if (!scheduled && !closing && !closed) {
			scheduled = true;
			steps.port2.postMessage(0);
		}
	}
	function cancelRust(through) {
		if (rustGeneration !== null && rustGeneration <= through) {
			// Rust requires the actual accepted generation, not an arbitrary cut.
			ffi(() => service.cancel(rustGeneration.toString()));
			rustGeneration = null;
		}
	}
	function retireThrough(through) {
		// Cancel first; no ACK claims affected Rust retirement if this throws.
		cancelRust(through);
		if (through > retiredThrough) retiredThrough = through;
		if (through > lastQueued) lastQueued = through;
		for (const ticket of [
			desired,
			validating,
			current,
			copyPending,
			finishing,
		]) {
			if (ticket && ticket.generation <= through) {
				ticket.cancelled = true;
				retired(ticket);
			}
		}
		if (desired?.generation <= through) desired = null;
		if (current?.generation <= through) current = null;
		if (copyPending?.generation <= through) copyPending = null;
	}
	function cleanupTicket(ticket, error) {
		if (ticket.responded || closing || closed) return;
		let complete = false;
		try {
			// Only this active ticket may clean its failed Rust preparation/step.
			if (
				current === ticket ||
				copyPending === ticket ||
				finishing === ticket
			) {
				if (
					!error7(detail(error), error?.[rustOrigin] === true)
						.completed
				)
					poisoned = true;
				ffi(() => service.retire());
				rustGeneration = null;
				if (current === ticket) current = null;
				if (copyPending === ticket) copyPending = null;
			}
			complete = true;
		} catch {
			poisoned = true;
		}
		ticket.responded = true;
		failure(ticket.id, error, complete);
	}
	function checkSubset(request) {
		const pairs = unionPairs(
			plan.sourceRequirements.pairs,
			request.operation,
		);
		return numericalDigests7(
			pairs,
			pairs.map((pair) => {
				const entry = entries.find(
					(e) =>
						e.profile_id === pair.profile_id &&
						e.codec_id === pair.codec_id,
				);
				if (!entry)
					throw fail(
						'CapabilityUnsupported',
						'Source7 operation outside final HELLO subset',
					);
				return entry;
			}),
		).then((actual) => {
			if (!equal7(actual, numericalFacts7(request.unionRequirements)))
				throw fail('CodecFailed', 'Source7 union subset binding');
		});
	}
	function validateDesired() {
		if (validating || !desired || closing || closed) return;
		const ticket = desired;
		desired = null;
		validating = ticket;
		const task = (async () => {
			try {
				ticket.request = await verifyBegin7(ticket.request, plan);
				await checkSubset(ticket.request);
				if (!live(ticket)) return;
				// Only fully verified matching new metadata retires prior Rust work.
				if (
					rustGeneration !== null &&
					rustGeneration < ticket.generation
				)
					cancelRust(rustGeneration);
				retired(current);
				current = ticket;
				schedule();
			} catch (error) {
				cleanupTicket(ticket, error);
			} finally {
				if (validating === ticket) validating = null;
				if (desired) validateDesired();
			}
		})();
		track(task);
	}
	async function pump() {
		scheduled = false;
		if (pumping || !current || closing || closed) return;
		pumping = true;
		const ticket = current;
		try {
			if (!live(ticket)) return;
			if (!ticket.binding) {
				const binding = json7(
					ffi(() =>
						service.prepare(
							encodeProgram(ticket.request, BEGIN_BYTES_MAX),
						),
					),
					BEGIN_BYTES_MAX,
				);
				rustGeneration = ticket.generation;
				ticket.binding = await verifyBinding7(
					binding,
					ticket.request,
					plan,
				);
				if (current !== ticket || !live(ticket)) return;
				if (!ticket.request.render) {
					post({ ...ticket.binding, id: ticket.id });
					ticket.responded = true;
					current = null;
					return;
				}
			}
			if (!ffi(() => service.step())) {
				schedule();
				return;
			}
			if (current !== ticket || !live(ticket)) return;
			if (ticket.copy) {
				const bytes = ffi(() =>
					service.encodedFrame(new Uint8Array(ticket.mask)),
				);
				if (
					!(bytes instanceof Uint8Array) ||
					bytes.byteLength < 1 ||
					bytes.byteLength > (16 << 20) - ticket.output.maskLength
				)
					throw fail('CodecFailed', 'Source7 actual encoded extent');
				pngExtent7(bytes, ticket.binding);
				ticket.mask = null;
				current = null;
				finishing = ticket;
				const task = (async () => {
					try {
						const bytesSha256 = await digestEncoded(bytes);
						sha7(bytesSha256);
						if (!live(ticket) || finishing !== ticket) return;
						post(
							{
								kind: 'encoded-copy',
								id: ticket.id,
								outputVersion: 1,
								outputSha256: ticket.copy.outputSha256,
								bytesSha256,
								mime: 'image/png',
								binding: ticket.binding,
								bytes,
							},
							[bytes.buffer],
						);
						ticket.responded = true;
					} catch (error) {
						cleanupTicket(ticket, error);
					} finally {
						if (finishing === ticket) finishing = null;
					}
				})();
				track(task);
			} else {
				const pixels = ffi(() => service.frame());
				if (
					!(pixels instanceof Uint8Array) ||
					pixels.byteLength !==
						ticket.binding.width * ticket.binding.height * 4
				)
					throw fail('CodecFailed', 'Source7 actual RGBA extent');
				post({ ...ticket.binding, id: ticket.id, pixels }, [
					pixels.buffer,
				]);
				ticket.responded = true;
				current = null;
			}
		} catch (error) {
			cleanupTicket(ticket, error);
		} finally {
			pumping = false;
			if (current) schedule();
		}
	}
	steps.port1.onmessage = () => {
		track(pump());
	};
	async function runtimeReady(data) {
		if (phase !== 'new')
			throw fail('ResourceLimit', 'Source7 readiness already started');
		phase = 'loading';
		await load();
		if (closing) return;
		ready = freeze7(
			json7(
				ffi(() => api.layer_appearance_ready(data.noise)),
				OPEN_BYTES_MAX,
			),
		);
		validateReady7(ready, data.noise ? 'noise' : 'base');
		phase = 'ready';
		post({ kind: 'runtime-ready', id: data.id, ...ready });
	}
	async function capability(data) {
		if (phase !== 'ready')
			throw fail('InputSchema', 'Source7 readiness must precede HELLO');
		const control = { ...data };
		delete control.id;
		delete control.kind;
		validateHello7(control);
		if (
			(control.admission.mode === 'noise') !==
			(ready.runtimeFacts.noiseSliceWork !== '0')
		)
			throw fail('ConflictStaleBase', 'Source7 getter/HELLO mode');
		phase = 'hello-loading';
		await load();
		if (closing) return;
		const reply = json7(
			ffi(() =>
				api.layer_appearance_capabilities(
					encodeProgram(control, CAPABILITY_BYTES_MAX),
				),
			),
			CAPABILITY_BYTES_MAX,
		);
		const expected = await numericalDigests7(
			control.pairs,
			control.pairs.map(numericalEntry7),
		);
		const actual = await validateCapabilities7(
			reply,
			control.pairs,
			expected,
			control.admission,
			ready.runtimeFacts,
		);
		if (closing) return;
		hello = freeze7(snapshot7(control));
		entries = actual;
		phase = 'hello';
		post({ ...reply, id: data.id });
	}
	async function open(data) {
		if (phase !== 'hello' || service)
			throw fail(
				'InputSchema',
				'Source7 final HELLO must precede one OPEN',
			);
		const bytes = sourceBuffer7(data.source, SOURCE_BYTES_MAX);
		validatePlan7(data.plan);
		phase = 'opening';
		const draft = snapshot7(data.plan, OPEN_BYTES_MAX);
		for (const key of ['appearance', 'admission', 'slice']) {
			if (!equal7(draft[key], hello[key]))
				throw fail('ConflictStaleBase', 'Source7 sealed HELLO/OPEN');
		}
		const subset = draft.sourceRequirements.pairs.map((pair) => {
			const entry = entries.find(
				(e) =>
					e.profile_id === pair.profile_id &&
					e.codec_id === pair.codec_id,
			);
			if (!entry)
				throw fail(
					'CapabilityUnsupported',
					'Source7 source outside HELLO subset',
				);
			return entry;
		});
		const verified = await verifyPlan7(draft, subset);
		await load();
		if (closing) return;
		try {
			service = ffi(() =>
				api.LayerAppearanceRenderWorker.open(
					new Uint8Array(bytes),
					encodeProgram(verified, OPEN_BYTES_MAX),
				),
			);
			const binding = json7(
				ffi(() => service.binding()),
				OPEN_BYTES_MAX,
			);
			validateContext7(binding, verified);
			plan = verified;
			phase = 'open';
			if (!closing) post({ kind: 'ready', id: data.id, binding });
		} catch (error) {
			phase = 'failed';
			if (service) {
				ffi(() => service.free());
				service = null;
			}
			throw error;
		}
		// Only immutable metadata survives. No bytes/source retained in state.
	}
	function ticket(data) {
		validateBegin7(data.request, plan);
		const generation = generation7(data.request.generation);
		if (generation <= lastQueued)
			throw fail(
				'ConflictStaleBase',
				'Source7 generation did not advance',
			);
		const result = {
			id: data.id,
			generation,
			request: freeze7(snapshot7(data.request, BEGIN_BYTES_MAX)),
			cancelled: false,
			responded: false,
			binding: null,
		};
		lastQueued = generation;
		return result;
	}
	function update(data) {
		if (copyPending || finishing)
			throw fail('ResourceLimit', 'Source7 visual copy owns slot');
		const next = ticket(data);
		// Coalesce pending JS metadata immediately; leave actual Rust work alone
		// until the new request's complete hashes/subset have been verified.
		for (const old of [desired, validating]) retired(old);
		desired = next;
		validateDesired();
	}
	async function prepareCopy(data) {
		if (current || desired || validating || copyPending || finishing)
			throw fail('ResourceLimit', 'Source7 visual slot busy');
		const next = ticket(data);
		const control = visualSpec7(data.output, next.request, plan);
		next.output = freeze7(snapshot7(data.output, 4096));
		copyPending = next;
		try {
			next.request = await verifyBegin7(next.request, plan);
			await checkSubset(next.request);
			const expected = await sha(new TextEncoder().encode(control));
			if (!live(next) || copyPending !== next) return;
			const admitted = json7(
				ffi(() =>
					service.prepareCopy(
						encodeProgram(next.request, BEGIN_BYTES_MAX),
						control,
					),
				),
				BEGIN_BYTES_MAX,
			);
			rustGeneration = next.generation;
			validateCopyAdmitted7(admitted, next.request, plan, expected);
			next.binding = await verifyBinding7(
				admitted.binding,
				next.request,
				plan,
			);
			if (!live(next) || copyPending !== next) return;
			next.copy = { outputSha256: admitted.outputSha256 };
			post({ ...admitted, id: data.id });
			next.responded = true;
		} catch (error) {
			cleanupTicket(next, error);
		}
	}
	function copyInput(data) {
		if (
			!copyPending?.copy ||
			current ||
			desired ||
			validating ||
			finishing ||
			data.outputSha256 !== copyPending.copy.outputSha256
		)
			throw fail('ConflictStaleBase', 'Source7 visual input authority');
		const mask = selection7(
			data.mask,
			copyPending.output.maskLength,
			data.outputSha256,
		);
		const next = { ...copyPending, id: data.id, responded: false, mask };
		copyPending = null;
		current = next;
		schedule();
	}
	function close(data) {
		if (closed) {
			post({ kind: 'closed', id: data.id });
			return Promise.resolve();
		}
		if (closingTask)
			throw fail('ResourceLimit', 'Source7 close already pending');
		closing = true;
		phase = 'closing';
		closingTask = (async () => {
			if (service) retireThrough(0xffffffffffffffffn);
			await Promise.allSettled([...tasks]);
			if (service) {
				ffi(() => service.free());
				service = null;
			}
			desired = null;
			validating = null;
			current = null;
			copyPending = null;
			finishing = null;
			plan = null;
			hello = null;
			entries = null;
			ready = null;
			steps.port1.close();
			steps.port2.close();
			closed = true;
			phase = 'closed';
			post({ kind: 'closed', id: data.id });
		})();
		return closingTask;
	}
	const fields = {
		'runtime-ready': ['noise'],
		hello: [
			'protocol',
			'registryContract',
			'pairs',
			'appearance',
			'admission',
			'slice',
		],
		open: ['source', 'plan'],
		update: ['request'],
		cancel: ['generation'],
		close: [],
		'copy-prepare': ['request', 'output'],
		'copy-input': ['outputSha256', 'mask'],
	};
	async function receive({ data }) {
		try {
			if (!Object.hasOwn(fields, data?.kind))
				throw fail('InputSchema', 'Unknown Source7 Worker operation');
			message7(data, fields[data.kind], lastId);
			lastId = data.id;
			if (data.kind === 'close') {
				await close(data);
				return;
			}
			if (closing || closed || poisoned)
				throw fail('CodecFailed', 'Source7 Worker closing or failed');
			if (data.kind === 'runtime-ready') {
				if (typeof data.noise !== 'boolean')
					throw fail('InputSchema', 'Source7 mode boolean');
				await track(runtimeReady(data));
				return;
			}
			if (data.kind === 'hello') {
				await track(capability(data));
				return;
			}
			if (data.kind === 'open') {
				await track(open(data));
				return;
			}
			if (!service || phase !== 'open')
				throw fail('InputSchema', 'Source7 not open');
			if (data.kind === 'update') update(data);
			else if (data.kind === 'copy-prepare')
				await track(prepareCopy(data));
			else if (data.kind === 'copy-input') copyInput(data);
			else if (data.kind === 'cancel') {
				retireThrough(generation7(data.generation));
				post({
					kind: 'retired',
					id: data.id,
					generation: data.generation,
				});
			}
		} catch (error) {
			failure(
				data?.id,
				error,
				!service &&
					['open', 'hello', 'runtime-ready'].includes(data?.kind),
			);
		}
	}
	return Object.freeze({ receive });
}
