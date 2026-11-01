// Transport/control unit tests. Stub bytes are not an actual codec/browser proof.
import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';
import {
	COPY_LIMITS,
	joinableCopy,
	makeEncodedCopy,
	validateCopyLimits,
	validateCopySpec,
	validateEncodedCopy,
} from '../src/lib/filter-preview-copy.js';
import { canonical, sha } from '../src/lib/filter-preview-protocol.js';

globalThis.crypto ??= webcrypto;
const uuid = '00000000-0000-0000-0000-000000000001';
const workload = {
	id: 'noise-source-work-v1',
	revision: 1,
	workspaceBytes: 67108864,
	baseWorkLimit: 268435456,
	noiseWorkLimit: 34359738368,
	totalWorkLimit: 34628173824,
	sha256: '66f46c5fd8e98527e7b1da485080498a9fb637d6c3d81f868fbdff117a78f5fd',
};
const facts = {
	count: 2,
	requiredSha256:
		'55648214762fa48c9e8c1470e6f8a46343b46c8f66df193028278ad39b219a74',
	resolutionSha256:
		'209af4ec3e29790211b135631926a9c9203a86ca0f50253a71a140f3db1b4395',
	capabilitySha256:
		'af8ab21a88343a5d050f8fc947550f5ebf4b990ce887669eedb3616935160ff2',
};
const plan = {
	protocol: 6,
	registryContract: 2,
	owner: { session: uuid, state: uuid, epoch: '0', frontend: 3 },
	draft: uuid,
	document: uuid,
	size: [2, 1],
	sourceSha256: 'a'.repeat(64),
	workload,
	sourceRequirements: {
		...facts,
		pairs: [
			{ profile_id: 'add-noise-rgb-v1', codec_id: 'params-v1' },
			{ profile_id: 'invert-rgb-v1', codec_id: 'params-v1' },
		],
	},
};
const options = {
	generation: '1',
	operation: null,
	target: null,
	render: true,
	view: { region: [0, 0, 2, 1], level: 0 },
	candidateRequirements: { ...facts },
	unionRequirements: { ...facts },
};
const spec = {
	version: 1,
	purpose: 'download',
	format: 'png',
	quality: 90,
	matte: [255, 255, 255],
	frontend: 'studio',
	omitMetadata: false,
	bakeFilters: true,
	maskLength: 0,
	maskSha256:
		'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
};
function request() {
	return {
		...options,
		owner: plan.owner,
		draft: plan.draft,
		sourceSha256: plan.sourceSha256,
		protocol: 6,
		registryContract: 2,
		workloadSha256: workload.sha256,
	};
}
async function binding() {
	return {
		kind: 'frame',
		generation: '1',
		owner: plan.owner,
		draft: plan.draft,
		sourceSha256: plan.sourceSha256,
		target: null,
		view: options.view,
		width: 2,
		height: 1,
		region: [0, 0, 2, 1],
		level: 0,
		operationSha256: await sha(new TextEncoder().encode('null')),
		sourceRequirements: { ...facts },
		candidateRequirements: { ...facts },
		unionRequirements: { ...facts },
		protocol: 6,
		registryContract: 2,
		workloadSha256: workload.sha256,
		workQuote: {
			workloadSha256: workload.sha256,
			base: 136,
			noise: 2048,
			total: 2184,
		},
	};
}
test('flat-output1 binds independent stdlib canonical options vector', async () => {
	validateCopySpec(spec, options, plan);
	assert.equal(
		await sha(new TextEncoder().encode(canonical(spec))),
		'2a96dcf932b018ae1fa449f273ab1131ac0c722a59a4fec86fd946fc4c5081ba',
	);
	for (const changed of [
		{ ...spec, version: 2 },
		{ ...spec, format: 'dds' },
		{ ...spec, quality: 0 },
		{ ...spec, matte: [1, 2, 3] },
		{ ...spec, purpose: 'print' },
		{ ...spec, maskLength: 3 },
		{ ...spec, extra: true },
	])
		assert.throws(() => validateCopySpec(changed, options, plan));
	assert.throws(() =>
		validateCopySpec(
			spec,
			{ ...options, view: { region: [0, 0, 2, 1], level: 1 } },
			plan,
		),
	);
});
test('flat-output actual artifact limits cannot enlarge old pools or price', () => {
	validateCopyLimits(COPY_LIMITS);
	for (const key of Object.keys(COPY_LIMITS))
		assert.throws(() =>
			validateCopyLimits({ ...COPY_LIMITS, [key]: COPY_LIMITS[key] + 1 }),
		);
	assert.throws(() => validateCopyLimits({ ...COPY_LIMITS, newPool: 1 }));
});
async function transport({ badLimits = false, cancelled = false } = {}) {
	const calls = [];
	let finished = 0,
		held = 0,
		marked = 0;
	const b = await binding();
	const digest = await sha(new TextEncoder().encode(canonical(spec)));
	const bytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 1]);
	const copy = makeEncodedCopy({
		plan,
		start: () => request(),
		finish: () => finished++,
		hold: () => held++,
		markUpdate: () => marked++,
		checkCurrent() {
			if (cancelled)
				throw Object.assign(Error('cancelled'), {
					code: 'OperationCancelled',
				});
		},
		async send(kind, body, transfer = []) {
			calls.push({ kind, body, transfer });
			if (kind === 'copy-prepare')
				return {
					kind: 'copy-admitted',
					id: 1,
					outputVersion: 1,
					outputSha256: digest,
					binding: b,
					limits: {
						...COPY_LIMITS,
						...(badLimits ? { outputBytes: 33554432 } : {}),
					},
				};
			return {
				kind: 'encoded-copy',
				id: 2,
				outputVersion: 1,
				outputSha256: digest,
				bytesSha256: await sha(bytes),
				mime: 'image/png',
				binding: b,
				bytes,
			};
		},
	});
	return { copy, calls, count: () => ({ finished, held, marked }) };
}
test('copy reservation ACK precedes input transfer and output ownership requires release', async () => {
	const t = await transport();
	const result = await t.copy(options, spec, new Uint8Array());
	assert.deepEqual(
		t.calls.map((c) => c.kind),
		['copy-prepare', 'copy-input'],
	);
	assert.equal(t.calls[0].transfer.length, 0);
	assert.equal(t.calls[1].transfer.length, 1);
	assert.deepEqual(t.count(), { finished: 0, held: 1, marked: 1 });
	result.release();
	result.release();
	assert.equal(t.count().finished, 1);
});
test('changed getter or cancellation refuses before any mask transfer', async () => {
	for (const mode of [{ badLimits: true }, { cancelled: true }]) {
		const t = await transport(mode);
		await assert.rejects(t.copy(options, spec, new Uint8Array()));
		assert.deepEqual(
			t.calls.map((c) => c.kind),
			mode.cancelled ? [] : ['copy-prepare'],
		);
		assert.equal(t.count().finished, 1);
		assert.equal(t.count().held, 0);
	}
});
test('encoded output cannot borrow source identity, codec, digest or work', async () => {
	const t = await transport();
	const valid = await t.copy(options, spec, new Uint8Array());
	const req = {
		...request(),
		operationSha256: await sha(new TextEncoder().encode('null')),
	};
	for (const changed of [
		{ ...valid, mime: 'image/jpeg' },
		{ ...valid, bytesSha256: '0'.repeat(64) },
		{ ...valid, binding: { ...valid.binding, generation: '2' } },
		{
			...valid,
			binding: {
				...valid.binding,
				workQuote: { ...valid.binding.workQuote, total: 1 },
			},
		},
	]) {
		delete changed.release;
		await assert.rejects(
			validateEncodedCopy(changed, req, plan, spec, valid.outputSha256),
		);
	}
	valid.release();
});
test('clipboard activation is consumed before encoded bytes arrive, without a canvas', async () => {
	const previous = {
		secure: globalThis.isSecureContext,
		navigator: globalThis.navigator,
		ClipboardItem: globalThis.ClipboardItem,
	};
	let item,
		writeCalls = 0;
	try {
		globalThis.isSecureContext = true;
		globalThis.ClipboardItem = class {
			constructor(value) {
				this.value = value;
			}
		};
		Object.defineProperty(globalThis, 'navigator', {
			configurable: true,
			value: {
				clipboard: {
					write(items) {
						writeCalls++;
						item = items[0];
						return item.value['image/png'].then(async (blob) => {
							assert.equal(blob.type, 'image/png');
							assert.equal(
								(await blob.arrayBuffer()).byteLength,
								8,
							);
						});
					},
				},
			},
		});
		const { beginDeferredPngClipboard } = await import(
			'../src/lib/clipboard-background.js?copy-unit'
		);
		const deferred = beginDeferredPngClipboard();
		assert.equal(writeCalls, 1);
		assert.throws(() => beginDeferredPngClipboard());
		deferred.resolve(new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]));
		await deferred.promise;
	} finally {
		globalThis.isSecureContext = previous.secure;
		globalThis.ClipboardItem = previous.ClipboardItem;
		Object.defineProperty(globalThis, 'navigator', {
			configurable: true,
			value: previous.navigator,
		});
	}
});

test('public client cancel/close joins pending byteSHA before slot release', async () => {
	const oldWorker = globalThis.Worker;
	const cryptoDescriptor = Object.getOwnPropertyDescriptor(
		globalThis,
		'crypto',
	);
	let releaseHash, notifyHash;
	const hashStarted = new Promise((resolve) => {
		notifyHash = resolve;
	});
	const hashBarrier = new Promise((resolve) => {
		releaseHash = resolve;
	});
	const sent = [];
	const realSha = async (bytes) =>
		[...new Uint8Array(await webcrypto.subtle.digest('SHA-256', bytes))]
			.map((v) => v.toString(16).padStart(2, '0'))
			.join('');
	const source = new Uint8Array([10, 11, 12]);
	const p = {
		...plan,
		sourceSha256: await realSha(source),
		layers: 1,
		residentBytes: 0,
		metadataBytes: 0,
		metadataCharge: 0,
		filterDigest: 'b'.repeat(64),
		maskCount: 0,
		maskBands: 0,
		maskRuns: 0,
		maskDecodeBytes: 0,
	};
	class TransportWorker {
		postMessage(message) {
			sent.push(message.kind);
			queueMicrotask(async () => {
				let data;
				if (message.kind === 'hello') {
					const entries = p.sourceRequirements.pairs.map(
						(pair, i) => ({
							...pair,
							resource_contract_id: i
								? 'direct-rgba8-point-v1'
								: 'direct-source-noise-rgba8-u512-g4096-v1',
							resource_contract_revision: 1,
							workload_class_id: i
								? 'base-work-v1'
								: 'noise-source-work-v1',
							workload_class_revision: 1,
						}),
					);
					data = {
						kind: 'capabilities',
						id: message.id,
						protocol: 6,
						registryContract: 2,
						entries,
						requiredSha256: facts.requiredSha256,
						resolutionSha256: facts.resolutionSha256,
						capabilitySha256: facts.capabilitySha256,
						workloadSha256: workload.sha256,
						runtimeFacts: {
							controlQuote: {
								retainedBytes: 4096,
								frameBytes: 1024,
								handleBytes: 12,
								extraOutputBytes: 16777216,
							},
							slicePolicy: {
								nativeEdge: 64,
								baseSliceLimit: 8388608,
								noiseSliceLimit: 134217728,
							},
						},
					};
				} else if (message.kind === 'open')
					data = {
						kind: 'ready',
						id: message.id,
						binding: {
							...message.plan,
							candidateRequirements: { ...facts },
							unionRequirements: { ...facts },
							workloadSha256: workload.sha256,
						},
					};
				else if (message.kind === 'copy-prepare') {
					this.outputSha = await realSha(
						new TextEncoder().encode(canonical(message.output)),
					);
					this.bound = {
						...(await binding()),
						owner: p.owner,
						draft: p.draft,
						sourceSha256: p.sourceSha256,
					};
					data = {
						kind: 'copy-admitted',
						id: message.id,
						outputVersion: 1,
						outputSha256: this.outputSha,
						binding: this.bound,
						limits: { ...COPY_LIMITS },
					};
				} else if (message.kind === 'copy-input') {
					const bytes = new Uint8Array([
						137, 80, 78, 71, 13, 10, 26, 10, 1,
					]);
					data = {
						kind: 'encoded-copy',
						id: message.id,
						outputVersion: 1,
						outputSha256: this.outputSha,
						bytesSha256: await realSha(bytes),
						mime: 'image/png',
						binding: this.bound,
						bytes,
					};
				} else if (message.kind === 'cancel')
					data = {
						kind: 'retired',
						id: message.id,
						generation: message.generation,
					};
				else if (message.kind === 'close')
					data = { kind: 'closed', id: message.id };
				else throw Error('unexpected transport message');
				this.onmessage({ data });
			});
		}
		terminate() {}
	}
	try {
		globalThis.Worker = TransportWorker;
		Object.defineProperty(globalThis, 'crypto', {
			configurable: true,
			value: {
				subtle: {
					async digest(algorithm, bytes) {
						const view = new Uint8Array(
							bytes.buffer ?? bytes,
							bytes.byteOffset ?? 0,
							bytes.byteLength,
						);
						if (view.length === 9 && view[8] === 1) {
							notifyHash();
							await hashBarrier;
						}
						return webcrypto.subtle.digest(algorithm, bytes);
					},
				},
			},
		});
		const service = await import(
			'../src/lib/filter-preview-worker.js?copy-hash-barrier'
		);
		const client = await service
			.beginFilterPreview(p)
			.open(source, p.sourceSha256);
		const result = client
			.encodedCopy(options, spec, new Uint8Array())
			.catch((error) => error);
		await hashStarted;
		await client.cancel('1');
		let closed = false;
		const close = client.close().then(() => {
			closed = true;
		});
		await new Promise((resolve) => setImmediate(resolve));
		assert.equal(closed, false);
		assert.equal(sent.includes('close'), false);
		assert.ok(service.filterPreviewUsage().reserved > 0);
		assert.throws(
			() => service.beginFilterPreview(p),
			(error) => error.code === 'ResourceLimit',
		);
		releaseHash();
		assert.equal((await result).code, 'OperationCancelled');
		await close;
		assert.equal(closed, true);
		assert.equal(service.filterPreviewUsage().reserved, 0);
		assert.equal(sent.filter((kind) => kind === 'close').length, 1);
	} finally {
		releaseHash?.();
		globalThis.Worker = oldWorker;
		if (cryptoDescriptor)
			Object.defineProperty(globalThis, 'crypto', cryptoDescriptor);
		else delete globalThis.crypto;
	}
});
test('copy finalization join waits and consumes rejection without hiding public failure', async () => {
	let release;
	const barrier = new Promise((resolve) => {
		release = resolve;
	});
	let joined = false;
	const pending = joinableCopy(async () => {
		await barrier;
		throw Object.assign(Error('cancelled'), { code: 'OperationCancelled' });
	});
	const result = pending.run().catch((error) => error);
	const join = pending.join().then(() => {
		joined = true;
	});
	await Promise.resolve();
	assert.equal(joined, false);
	await assert.rejects(
		pending.run(),
		(error) => error.code === 'ResourceLimit',
	);
	release();
	assert.equal((await result).code, 'OperationCancelled');
	await join;
	assert.equal(joined, true);
});

test('early clipboard denial plus cancellation cannot leave an unhandled payload rejection', async () => {
	const previous = {
		secure: globalThis.isSecureContext,
		navigator: globalThis.navigator,
		ClipboardItem: globalThis.ClipboardItem,
	};
	const unhandled = [];
	const observe = (error) => unhandled.push(error);
	process.on('unhandledRejection', observe);
	try {
		globalThis.isSecureContext = true;
		globalThis.ClipboardItem = class {
			constructor(value) {
				this.value = value;
			}
		};
		const denied = Error('permission denied before Blob consumption');
		Object.defineProperty(globalThis, 'navigator', {
			configurable: true,
			value: {
				clipboard: {
					write() {
						return Promise.reject(denied);
					},
				},
			},
		});
		const { beginDeferredPngClipboard } = await import(
			'../src/lib/clipboard-background.js?early-denial'
		);
		const deferred = beginDeferredPngClipboard();
		await assert.rejects(deferred.promise, (error) => error === denied);
		deferred.cancel();
		await new Promise((resolve) => setImmediate(resolve));
		assert.deepEqual(unhandled, []);
	} finally {
		process.off('unhandledRejection', observe);
		globalThis.isSecureContext = previous.secure;
		globalThis.ClipboardItem = previous.ClipboardItem;
		Object.defineProperty(globalThis, 'navigator', {
			configurable: true,
			value: previous.navigator,
		});
	}
});
