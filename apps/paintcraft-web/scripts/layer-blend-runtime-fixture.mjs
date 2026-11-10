// Explicit ABI contract double. This does NOT emulate/qualify a Rust renderer.
import assert from 'node:assert/strict';
import { VERSIONS8, LIMITS8 } from '../src/lib/layer-blend-preview-common.js';
import { createLayerBlendRuntime } from '../src/lib/layer-blend-preview-dispatch.js';
export const PROFILE =
	'ba3d84c361e599f2d9245c529608a1670b3e68831cecf9aa0d9cbb97a6ff41eb';
export const PIXELS = [20, 60, 100, 64, 0, 0, 0, 0];
const uuid = (n) =>
	`${n.repeat(8)}-${n.repeat(4)}-${n.repeat(4)}-${n.repeat(4)}-${n.repeat(12)}`;
export function manifest() {
	return {
		kind: 'open',
		protocol: 8,
		id: 1,
		owner: uuid('1'),
		publicTicket: { state: uuid('2'), epoch: '19' },
		document: uuid('3'),
		sourceState: uuid('4'),
		draft: uuid('5'),
		sourceSha256: 'a'.repeat(64),
		profileSha256: PROFILE,
		canvas: [2, 1],
		byteLength: '4',
		storedNonNormal: true,
		storedNoise: false,
		mode: 'base',
		roles: {
			E: '4096',
			P: '1048576',
			M: '1048576',
			A: '1048576',
			I: '0',
			C: '1048576',
			Q: '0',
			K: '4096',
			W: '4194304',
			F: '16384',
			O: '32992',
			H: '7168',
		},
		pairedBytes: '0',
		baseWork: '268435456',
		noiseWork: '0',
	};
}
const typed = (phase) =>
	JSON.stringify({
		code: 'codec.failed',
		message: `contract ${phase} failure`,
		operationIndex: 7,
	});
export function fixture(failure = '') {
	const memory = new WebAssembly.Memory({ initial: 32, maximum: 4096 });
	const events = [];
	let live = 0;
	class ContractService {
		constructor() {
			events.push('constructor');
			if (failure === 'constructor') throw typed('constructor');
			if (failure === 'constructor-trap')
				throw new WebAssembly.RuntimeError('contract trap');
			live++;
			this.accepted = false;
		}
		controlPtr() {
			return 1024;
		}
		controlLen() {
			return 512;
		}
		stage() {
			events.push('stage');
			assert.equal(new DataView(memory.buffer).getUint8(1030), 1);
			if (failure === 'stage') throw typed('stage');
			this.accepted = true;
			return 0;
		}
		sourcePtr() {
			events.push('sourcePtr');
			return 4096;
		}
		sourceLen() {
			events.push('sourceLen');
			if (failure === 'grow-before-fill') memory.grow(1);
			return 4;
		}
		seal() {
			events.push('seal');
			assert.deepEqual(
				[...new Uint8Array(memory.buffer, 4096, 4)],
				[1, 2, 3, 4],
			);
			if (failure === 'seal') throw typed('seal');
			return 1;
		}
		beginView() {
			events.push('beginView');
			const d = new DataView(memory.buffer, 1024, 512);
			this.token = d.getBigUint64(304, true);
			this.generation = d.getBigUint64(312, true);
			return 2;
		}
		step() {
			events.push('step');
			new Uint8Array(memory.buffer, 8192, 8).set(PIXELS);
			return 3;
		}
		patchFact(i) {
			return [
				0n,
				0n,
				0n,
				2n,
				1n,
				this.token,
				this.generation,
				2n,
				1n,
				1n,
			][i];
		}
		rgbaPtr() {
			events.push('rgbaPtr');
			return 8192;
		}
		rgbaLen() {
			events.push('rgbaLen');
			return 8;
		}
		ack() {
			events.push('ack');
			return 4;
		}
		cancel() {
			events.push('cancel');
			return 5;
		}
		workUsed(noise) {
			return noise ? 0n : 123n;
		}
		hasOpenManifest() {
			return this.accepted;
		}
		close() {
			events.push('close');
			const bytes = new Uint8Array(memory.buffer, 1024, 512),
				d = new DataView(bytes.buffer, bytes.byteOffset, 512);
			assert.equal(bytes[6], 5);
			assert(bytes.subarray(304).every((n) => n === 0));
			if (!this.accepted)
				assert(bytes.subarray(16).every((n) => n === 0));
			assert.equal(d.getUint32(8, true) > 0, true);
			live--;
			if (failure === 'close') throw typed('close');
			return 6;
		}
		free() {
			assert.fail('consuming close must never be followed by free');
		}
	}
	const facts = [...Object.values(VERSIONS8), ...Object.values(LIMITS8)].map(
		BigInt,
	);
	const profile = Buffer.from(PROFILE, 'hex');
	const api = {
		LayerBlendRenderWorker: ContractService,
		layer_blend_runtime_fact: (i) => facts[i],
		layer_blend_control_fact: (i) => [1024n, 2048n, 4096n][i],
		layer_blend_profile_word: (i) => profile.readUInt32LE(i * 4),
		layer_blend_live_instances: () => live,
	};
	class ContractWorker {
		constructor() {
			this.terminated = false;
			this.runtime = createLayerBlendRuntime({
				initialize: async () => ({ memory }),
				api,
				endpoint: {
					postMessage: (value, transfer = []) => {
						events.push(`message:${value.kind}`);
						const data = structuredClone(value, { transfer });
						queueMicrotask(() => this.onmessage?.({ data }));
					},
				},
			});
		}
		postMessage(value, transfer = []) {
			if (this.terminated) throw new Error('contract Worker terminated');
			const data = structuredClone(value, { transfer });
			queueMicrotask(() =>
				this.runtime.receive({ data }).catch(() => {}),
			);
		}
		terminate() {
			this.terminated = true;
			events.push('terminate');
		}
	}
	return { api, memory, events, Worker: ContractWorker, live: () => live };
}
