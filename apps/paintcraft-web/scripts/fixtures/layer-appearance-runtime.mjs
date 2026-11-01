// Synthetic Node fixtures only. No real Wasm/PCND/renderer qualification.
import { deflateSync } from 'node:zlib';
import {
	APPEARANCE7,
	admissionDigest7,
	capabilityDigest7,
	facts7,
	geometry7,
	numericalDigests7,
	operationDigest7,
	slice7,
	unionDigest7,
} from '../../src/lib/filter-preview-appearance-protocol.js';
import { sha } from '../../src/lib/filter-preview-protocol.js';
import { createLayerAppearanceRuntime } from '../../src/lib/layer-appearance-preview-dispatch.js';
export function deferred() {
	let resolve, reject;
	const promise = new Promise((a, b) => {
		resolve = a;
		reject = b;
	});
	return { promise, resolve, reject };
}
export async function fixture() {
	const admission = {
		mode: 'base',
		workAuthority: 'standalone-view-jobs',
		workspaceBytes: '67108864',
		baseWorkLimit: '268435456',
		noiseWorkLimit: '0',
		totalWorkLimit: '268435456',
	};
	const numerical = await numericalDigests7([], []);
	const fact = {
		...numerical,
		layerMasks: {
			stored: 1,
			enabled: 1,
			bands: 1,
			runs: 1,
			decodeUpperBytes: '128',
			identitySha256: 'aa'.repeat(32),
			coverageQuoteRevision: 1,
			coverageAbi: 'wasm32-v1',
		},
		sourceCoverageSha256: 'bb'.repeat(32),
		maskInventorySha256: 'cc'.repeat(32),
		capabilitySha256: '00'.repeat(32),
	};
	fact.capabilitySha256 = await capabilityDigest7(fact, admission);
	const plan = {
		protocol: 7,
		registryContract: 3,
		owner: {
			session: '11111111-1111-1111-1111-111111111111',
			state: '22222222-2222-2222-2222-222222222222',
			epoch: '0',
			frontend: 3,
		},
		draft: '33333333-3333-3333-3333-333333333333',
		document: '44444444-4444-4444-4444-444444444444',
		sourceSha256: '11'.repeat(32),
		size: [2, 1],
		layers: 1,
		residentBytes: 0,
		metadataBytes: 0,
		metadataCharge: 0,
		filterDigest: '22'.repeat(32),
		maskCount: 0,
		maskBands: 0,
		maskRuns: 0,
		maskDecodeBytes: 0,
		sourceRequirements: { ...fact, pairs: [] },
		appearance: { ...APPEARANCE7 },
		admission,
		slice: slice7('base'),
	};
	const union = {
		...numerical,
		sourceCoverageSha256: fact.sourceCoverageSha256,
		capabilitySha256: '00'.repeat(32),
	};
	union.capabilitySha256 = await unionDigest7(union, admission);
	async function request(generation, render = true) {
		return {
			protocol: 7,
			registryContract: 3,
			owner: structuredClone(plan.owner),
			draft: plan.draft,
			sourceSha256: plan.sourceSha256,
			generation: String(generation),
			operation: null,
			target: null,
			view: { region: [0, 0, 2, 1], level: 0 },
			render,
			candidateRequirements: facts7(plan.sourceRequirements),
			unionRequirements: structuredClone(union),
			admissionSha256: await admissionDigest7(admission),
		};
	}
	async function binding(r) {
		return {
			kind: r.render ? 'frame' : 'prepared',
			protocol: 7,
			registryContract: 3,
			generation: r.generation,
			owner: structuredClone(r.owner),
			draft: r.draft,
			sourceSha256: r.sourceSha256,
			target: r.target,
			view: structuredClone(r.view),
			...geometry7(r.view, plan.size),
			operationSha256: await operationDigest7(r.operation),
			admissionSha256: r.admissionSha256,
			sourceRequirements: facts7(plan.sourceRequirements),
			candidateRequirements: structuredClone(r.candidateRequirements),
			unionRequirements: structuredClone(r.unionRequirements),
			workQuote: {
				baseWork: '1000',
				noiseWork: '0',
				totalWork: '1000',
				maskWorkActual: '48',
				maskWorkUpper: '64',
				maskWorkMargin: '16',
			},
		};
	}
	const hello = {
		protocol: 7,
		registryContract: 3,
		pairs: [],
		appearance: { ...APPEARANCE7 },
		admission,
		slice: slice7('base'),
	};
	return { plan, hello, numerical, request, binding };
}
function crc(bytes) {
	let c = 0xffffffff;
	for (const b of bytes) {
		c ^= b;
		for (let i = 0; i < 8; i++) c = (c >>> 1) ^ (c & 1 ? 0xedb88320 : 0);
	}
	return (c ^ 0xffffffff) >>> 0;
}
function chunk(kind, bytes) {
	const data = Buffer.concat([Buffer.from(kind), bytes]);
	const header = Buffer.alloc(4),
		footer = Buffer.alloc(4);
	header.writeUInt32BE(bytes.length);
	footer.writeUInt32BE(crc(data));
	return Buffer.concat([header, data, footer]);
}
function png() {
	const h = Buffer.alloc(13);
	h.writeUInt32BE(2, 0);
	h.writeUInt32BE(1, 4);
	h[8] = 8;
	h[9] = 6;
	return new Uint8Array(
		Buffer.concat([
			Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
			chunk('IHDR', h),
			chunk(
				'IDAT',
				deflateSync(
					Buffer.from([0, 20, 115, 200, 32, 20, 115, 200, 32]),
				),
			),
			chunk('IEND', Buffer.alloc(0)),
		]),
	);
}
export async function harness(options = {}) {
	const f = await fixture();
	const events = [],
		calls = [],
		queue = [],
		bindings = new Map();
	const runtimeFacts = {
		protocol: 7,
		registryContract: 3,
		appearanceRevision: 1,
		queryRevision: 1,
		coverageAbi: 'wasm32-v1',
		sliceRevision: 1,
		nativeEdge: 64,
		baseSliceWork: '8388608',
		noiseSliceWork: '0',
		controlQuoteBytes: '1048576',
	};
	let instance,
		nextId = 0;
	class Channel {
		constructor() {
			this.port1 = {
				onmessage: null,
				close: () => calls.push('port1-close'),
			};
			this.port2 = {
				postMessage: () => queue.push(() => this.port1.onmessage()),
				close: () => calls.push('port2-close'),
			};
		}
	}
	class MockRust {
		static open(bytes, control) {
			calls.push(['open', [...bytes]]);
			instance = new MockRust(JSON.parse(control));
			return instance;
		}
		constructor(plan) {
			this.plan = plan;
			this.generation = null;
			this.bindingValue = null;
			this.remaining = 0;
		}
		binding() {
			return JSON.stringify(this.bindingValue ?? this.plan);
		}
		prepare(control) {
			const r = JSON.parse(control);
			calls.push(['prepare', r.generation]);
			if (options.prepareError) throw options.prepareError;
			this.generation = r.generation;
			this.bindingValue = bindings.get(r.generation);
			this.remaining = options.steps ?? 1;
			return JSON.stringify(this.bindingValue);
		}
		step() {
			calls.push(['step', this.generation]);
			return --this.remaining <= 0;
		}
		frame() {
			const pixels = new Uint8Array(
				options.frameBytes ?? [20, 115, 200, 32, 20, 115, 200, 32],
			);
			calls.push(['frame', this.generation, pixels.buffer]);
			return pixels;
		}
		cancel(generation) {
			calls.push(['cancel', generation]);
			if (generation !== this.generation)
				throw JSON.stringify({
					code: 'conflict.stale_base',
					message: 'mock actual accepted generation',
				});
			this.remaining = 0;
		}
		retire() {
			calls.push('retire');
			this.remaining = 0;
		}
		free() {
			calls.push('free');
		}
		prepareCopy(control, output) {
			const binding = JSON.parse(this.prepare(control));
			return JSON.stringify({
				kind: 'copy-admitted',
				outputVersion: 1,
				outputSha256: options.copySha,
				binding,
			});
		}
		encodedFrame(mask) {
			const bytes = png();
			calls.push(['encoded', [...mask], bytes.buffer]);
			return bytes;
		}
	}
	const api = {
		LayerAppearanceRenderWorker: MockRust,
		linear_memory_ceiling: () => 256 << 20,
		layer_appearance_protocol_version: () => 7,
		layer_appearance_ready: () =>
			JSON.stringify({ runtimeFacts, controlQuote: '1048576' }),
		layer_appearance_capabilities: () =>
			JSON.stringify({
				kind: 'capabilities',
				protocol: 7,
				registryContract: 3,
				entries: [],
				appearance: { ...APPEARANCE7 },
				admissionSha256: options.admissionSha,
				requiredSha256: f.numerical.requiredSha256,
				resolutionSha256: f.numerical.resolutionSha256,
				runtimeFacts,
			}),
	};
	options.admissionSha = await admissionDigest7(f.plan.admission);
	const runtime = createLayerAppearanceRuntime({
		initialize:
			options.initialize ??
			(() => Promise.resolve({ memory: { buffer: new ArrayBuffer(8) } })),
		api,
		endpoint: {
			postMessage(value, transfer) {
				calls.push(['post', value.kind, value.id]);
				events.push(structuredClone(value, { transfer }));
			},
		},
		Channel,
		...(options.digestEncoded
			? { digestEncoded: options.digestEncoded }
			: {}),
	});
	function send(kind, fields = {}) {
		return runtime.receive({ data: { kind, id: ++nextId, ...fields } });
	}
	async function register(generation, render = true) {
		const r = await f.request(generation, render);
		bindings.set(r.generation, await f.binding(r));
		return r;
	}
	async function turn() {
		if (queue.length) queue.shift()();
		await new Promise((resolve) => setImmediate(resolve));
	}
	async function drain() {
		for (let i = 0; i < 30; i++) {
			await turn();
			if (!queue.length)
				await new Promise((resolve) => setImmediate(resolve));
		}
	}
	async function until(predicate) {
		const deadline = Date.now() + 5000;
		while (!predicate()) {
			if (Date.now() > deadline)
				throw new Error('bounded mock condition not reached');
			await turn();
			await new Promise((resolve) => setTimeout(resolve, 1));
		}
	}
	async function start() {
		await send('runtime-ready', { noise: false });
		await send('hello', f.hello);
		await send('open', {
			source: new Uint8Array([7, 8, 9]).buffer,
			plan: f.plan,
		});
	}
	return {
		...f,
		runtime,
		api,
		calls,
		events,
		queue,
		bindings,
		runtimeFacts,
		send,
		register,
		turn,
		drain,
		until,
		start,
		get sequence() {
			return nextId;
		},
		get instance() {
			return instance;
		},
	};
}
export async function visualOutput(purpose = 'print') {
	return {
		version: 1,
		purpose,
		format: 'png',
		quality: 90,
		matte: [255, 255, 255],
		frontend: 'studio',
		omitMetadata: false,
		bakeFilters: false,
		maskLength: 0,
		maskSha256: await sha(new Uint8Array()),
	};
}
