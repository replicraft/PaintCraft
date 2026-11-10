// Genuine ABI consumer. No Main application routing is installed here.
import {
	copy8,
	shape8,
	uint8,
	decimal8,
	fail8,
} from './layer-blend-preview-common.js';
import {
	validateOpen8,
	validateSourceBuffer8,
	validateView8,
	geometry8,
	patchRect8,
	validateAck8,
	validateCancel8,
	validateClose8,
	validatePatch8,
	validateComplete8,
} from './layer-blend-preview-protocol.js';
import {
	compiledFacts8,
	getterProfile8,
	profile8,
	hBytes8,
	window8,
	pointer8,
	writeWindow8,
	typedRustError8,
	expectStatus8,
	getterU648,
	verifyOpenCompiled8,
} from './layer-blend-preview-wire.js';
import { binding8, sourceIdentity8 } from './layer-blend-preview-failure.js';

function fillSource(service, memory, source) {
	const pointer = service.sourcePtr(),
		length = service.sourceLen();
	pointer8(memory, pointer, length, source.byteLength);
	const input = new Uint8Array(source),
		target = new Uint8Array(memory.buffer, pointer, length);
	for (let at = 0; at < length; at += 65536)
		target.set(input.subarray(at, Math.min(at + 65536, length)), at);
	// No Rust reentry or await occurs until these borrowed views have retired.
}
function copyPatch(service, memory, length) {
	const pointer = service.rgbaPtr(),
		actual = service.rgbaLen();
	pointer8(memory, pointer, actual, length);
	const owned = new ArrayBuffer(length);
	new Uint8Array(owned).set(new Uint8Array(memory.buffer, pointer, length));
	return owned;
}
export function createLayerBlendRuntime({
	initialize,
	api,
	endpoint,
	crypto = globalThis.crypto,
}) {
	let memory,
		ready,
		profile,
		service = null,
		open = null,
		view = null;
	let lastId = 0,
		lastToken = 0n,
		lastGeneration = 0n,
		ordinal = 0;
	let phase = 'loading',
		failed = false,
		pending = false;
	let queued = 0,
		intakeSeen = false;
	let tail = Promise.resolve();
	const post = (value, transfer = []) =>
		endpoint.postMessage(value, transfer);
	const loading = (async () => {
		const wasm = await initialize();
		memory = wasm.memory;
		if (!memory || !(memory.buffer instanceof ArrayBuffer))
			fail8('CodecFailed', 'Source8 actual nonshared memory');
		ready = compiledFacts8(api);
		hBytes8(ready.runtimeFacts);
		profile = await profile8(ready.runtimeFacts, crypto);
		if (getterProfile8(api) !== profile)
			fail8('CodecFailed', 'Source8 actual compiled profile');
		phase = 'ready';
		post(ready);
	})();
	loading.catch(() => {
		failed = true;
		post({ kind: 'unjoined', protocol: 8 });
	});
	function request(m) {
		uint8(m.id, 0xffffffff, 1);
		if (m.id <= lastId || phase === 'closed' || failed)
			fail8('ConflictStaleBase', 'Source8 runtime request order');
	}
	function binary(kind, m, original = open) {
		writeWindow8(
			service,
			memory,
			window8(
				kind,
				m.id,
				original,
				view ? { ...view, ordinal: m.ordinal ?? 0 } : undefined,
			),
		);
	}
	function cleanup(id) {
		if (!service) return false;
		const accepted = service.hasOpenManifest();
		writeWindow8(
			service,
			memory,
			window8('close', id, accepted ? open : null),
		);
		const consumed = service;
		service = null;
		expectStatus8(consumed.close(), 6);
		phase = 'closed';
		view = null;
		pending = false;
		return true;
	}
	function step() {
		if (phase !== 'viewing' || pending)
			fail8('ConflictStaleBase', 'Source8 one pending Rust patch');
		expectStatus8(service.step(), 3);
		const facts = Array.from({ length: 10 }, (_, i) =>
			getterU648(service.patchFact(i)),
		);
		const expected = patchRect8(view, open.canvas, ordinal),
			g = geometry8(view.visible, view.level, open.canvas);
		const wanted = [
			BigInt(ordinal),
			...expected.map(BigInt),
			BigInt(view.token),
			BigInt(view.generation),
			...g.size.map(BigInt),
			BigInt(g.patches),
		];
		if (facts.some((n, i) => n !== wanted[i]))
			fail8('CodecFailed', 'Source8 actual Rust patch facts');
		const n = (expected[2] - expected[0]) * (expected[3] - expected[1]) * 4;
		if (n > 16384 || 2n * BigInt(n) + 224n > decimal8(open.roles.O))
			fail8('ResourceLimit', 'Source8 original O transport overlap');
		const rgba = copyPatch(service, memory, n);
		const packet = {
			kind: 'patch',
			protocol: 8,
			id: view.id,
			...binding8(open, view),
			ordinal,
			patch: expected,
			rgba,
		};
		validatePatch8(packet, view, open, ordinal);
		pending = true;
		post(packet, [rgba]);
	}
	async function handle(data) {
		await loading;
		shape8(data, ['control', 'source']);
		const m = data.control;
		request(m);
		if (m.kind === 'close-empty') {
			shape8(m, ['kind', 'protocol', 'id']);
			if (
				m.protocol !== 8 ||
				open ||
				service ||
				phase !== 'ready' ||
				data.source !== null
			)
				fail8('ConflictStaleBase', 'Source8 empty close phase');
			lastId = m.id;
			phase = 'closed';
			post({ kind: 'closed-empty', protocol: 8, id: m.id });
			return;
		}
		let source = data.source;
		// Do not keep the exclusive transferred source rooted in the event object.
		data.source = null;
		let constructorAttempt = false;
		const failureView = view;
		try {
			if (m.kind === 'open') {
				if (phase !== 'ready' || open)
					fail8('ConflictStaleBase', 'Source8 OPEN once');
				const captured = validateOpen8(m);
				verifyOpenCompiled8(captured, ready.runtimeFacts, profile);
				validateSourceBuffer8(source, captured);
				open = captured;
				lastId = m.id;
				constructorAttempt = true;
				service = new api.LayerBlendRenderWorker();
				constructorAttempt = false;
				binary('open', m);
				expectStatus8(service.stage(), 0);
				fillSource(service, memory, source);
				source = null;
				expectStatus8(service.seal(), 1);
				phase = 'opened';
				post({
					kind: 'opened',
					protocol: 8,
					id: m.id,
					...sourceIdentity8(open),
					byteLength: open.byteLength,
					canvas: open.canvas,
					storedNonNormal: open.storedNonNormal,
					storedNoise: open.storedNoise,
				});
				return;
			}
			if (source !== null)
				fail8('InputSchema', 'Source8 only OPEN carries bytes');
			if (!open || !service)
				fail8('ConflictStaleBase', 'Source8 no source instance');
			if (m.kind === 'view') {
				const captured = validateView8(m, open);
				if (
					!['opened', 'complete', 'cancelled'].includes(phase) ||
					BigInt(m.token) <= lastToken ||
					BigInt(m.generation) <= lastGeneration
				)
					fail8(
						'ConflictStaleBase',
						'Source8 new dispatched operation only',
					);
				view = captured;
				lastId = m.id;
				lastToken = BigInt(m.token);
				lastGeneration = BigInt(m.generation);
				ordinal = 0;
				pending = false;
				binary('view', m);
				expectStatus8(service.beginView(), 2);
				phase = 'viewing';
				step();
				return;
			}
			if (m.kind === 'ack') {
				validateAck8(m, view, open, ordinal);
				if (!pending || phase !== 'viewing')
					fail8('ConflictStaleBase', 'Source8 ACK phase');
				binary('ack', m);
				const status = service.ack();
				lastId = m.id;
				pending = false;
				ordinal++;
				if (status === 4) {
					const packet = {
						kind: 'complete',
						protocol: 8,
						id: view.id,
						...binding8(open, view),
						patchCount: ordinal,
					};
					validateComplete8(packet, view, open);
					phase = 'complete';
					post(packet);
				} else {
					expectStatus8(status, 2);
					step();
				}
				return;
			}
			if (m.kind === 'cancel') {
				validateCancel8(m, view, open);
				if (phase !== 'viewing')
					fail8('ConflictStaleBase', 'Source8 CANCEL phase');
				binary('cancel', m);
				expectStatus8(service.cancel(), 5);
				lastId = m.id;
				pending = false;
				phase = 'cancelled';
				post({
					kind: 'cancelled',
					protocol: 8,
					id: m.id,
					...binding8(open, view),
				});
				return;
			}
			validateClose8(m, open, lastToken.toString());
			lastId = m.id;
			cleanup(m.id);
			post({
				kind: 'closed',
				protocol: 8,
				id: m.id,
				...sourceIdentity8(open),
				token: m.token,
			});
		} catch (error) {
			source = null;
			pending = false;
			const detail = typedRustError8(error);
			if (!detail || !open) {
				failed = true;
				throw error;
			}
			const errorBinding = binding8(
				open,
				m.kind === 'view' ? view : failureView,
			);
			let joined = false;
			try {
				if (service) joined = cleanup(m.id + 1);
				else if (
					constructorAttempt &&
					api.layer_blend_live_instances() === 0
				) {
					phase = 'closed';
					joined = true;
				}
			} catch {
				joined = false;
			}
			if (!joined) {
				failed = true;
				throw error;
			}
			post({
				kind: 'error',
				protocol: 8,
				id: m.id,
				phase: m.kind,
				closed: true,
				detail,
				binding: errorBinding,
			});
		}
	}
	return Object.freeze({
		receive({ data }) {
			// Validate and detach metadata BEFORE the first asynchronous boundary.
			// This queue owns at most two control packets and one source intake.
			let packet;
			try {
				shape8(data, ['control', 'source']);
				if (!Object.getOwnPropertyDescriptor(data, 'source').writable)
					fail8(
						'InputSchema',
						'Source8 owned writable intake envelope',
					);
				const m = data.control;
				if (queued >= 2)
					fail8('ResourceLimit', 'Source8 bounded dispatch queue');
				if (m?.kind === 'open') {
					if (intakeSeen)
						fail8(
							'ConflictStaleBase',
							'Source8 one transferred source',
						);
					packet = { control: validateOpen8(m), source: data.source };
					validateSourceBuffer8(packet.source, packet.control);
					intakeSeen = true;
				} else {
					if (data.source !== null)
						fail8(
							'InputSchema',
							'Source8 unexpected source buffer',
						);
					if (m?.kind === 'close-empty') {
						shape8(m, ['kind', 'protocol', 'id']);
						uint8(m.id, 0xffffffff, 1);
					} else if (m?.kind === 'view') validateView8(m, open);
					else if (m?.kind === 'ack')
						validateAck8(m, view, open, ordinal);
					else if (m?.kind === 'cancel')
						validateCancel8(m, view, open);
					else validateClose8(m, open, lastToken.toString());
					packet = { control: copy8(m), source: null };
				}
				data.source = null;
			} catch (error) {
				if (
					data &&
					Object.getOwnPropertyDescriptor(data, 'source')?.writable
				)
					data.source = null;
				failed = true;
				post({ kind: 'unjoined', protocol: 8 });
				return Promise.reject(error);
			}
			queued++;
			const task = tail.then(async () => {
				try {
					await handle(packet);
				} finally {
					queued--;
					packet = null;
				}
			});
			tail = task.catch((error) => {
				failed = true;
				endpoint.postMessage({ kind: 'unjoined', protocol: 8 });
				// Main quarantines. No successful-close or refund is inferred here.
			});
			return task;
		},
	});
}
