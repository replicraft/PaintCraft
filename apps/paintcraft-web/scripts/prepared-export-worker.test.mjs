// Synthetic wire/service controls, not real codec or browser execution.
import assert from 'node:assert/strict';
import test from 'node:test';
import * as dds from '../src/lib/dds-prepared-worker-protocol.js';
import * as msp from '../src/lib/msp-prepared-worker-protocol.js';
import * as oldDds from '../src/lib/dds-worker-protocol.js';
import * as oldMsp from '../src/lib/msp-worker-protocol.js';
import { context } from './test-support/source-metadata-context.mjs';
const owner = {
		session: '00000000-0000-4000-8000-000000000001',
		state: '00000000-0000-4000-8000-000000000002',
		epoch: 7n,
		frontend: 3,
	},
	nonce = '00000000-0000-4000-8000-000000000003',
	version = '0.1.0';
function spatial() {
	const b = new Uint8Array(64),
		v = new DataView(b.buffer);
	v.setUint16(0, 6);
	v.setUint16(2, 2);
	v.setUint32(4, 0);
	v.setUint32(8, 0);
	v.setBigUint64(16, 1n);
	v.setBigUint64(24, 0n);
	v.setBigUint64(32, 100n);
	v.setBigUint64(40, 2000000n);
	return b;
}
function programs() {
	const b = new Uint8Array(128),
		v = new DataView(b.buffer);
	v.setUint16(0, 1);
	v.setUint16(2, 1);
	v.setUint32(4, 1);
	v.setUint32(8, 1);
	v.setUint32(12, 1);
	v.setUint32(16, 1);
	for (const [at, value] of [
		[24, 1000n],
		[32, 264n],
		[40, 8192n],
		[48, 264n],
		[56, 1100n],
	])
		v.setBigUint64(at, value);
	b.fill(31, 64, 96);
	b.fill(41, 96, 128);
	return b;
}

function plan(codec) {
	return {
		width: 1,
		height: 1,
		sourceFlags: 4,
		bakeIntent: 1,
		filterDigest: new Uint8Array(32).fill(17),
		maskContext: new Uint8Array(32),
		metadataContext: context(codec === 'dds' ? 1 : 2),
		protocolVersion: 7,
		spatialContext: spatial(),
		programContext: programs(),
		...(codec === 'dds'
			? { settings: new Uint8Array([0, 0, 0, 1, 0, 0, 0, 0]) }
			: { version: 2 }),
	};
}
async function response(codec, header) {
	const encoded = new Uint8Array([1, 2, 3]),
		preview = new Uint8Array([0, 0, 0, 255]),
		out = new Uint8Array(608 + encoded.length + preview.length),
		v = new DataView(out.buffer),
		text = new TextEncoder();
	out.set(text.encode(codec === 'dds' ? 'PCDDSR07' : 'PCMSPR07'));
	v.setUint16(8, 7);
	out.set(header.subarray(16, 24), 12);
	out.set(header.subarray(12, 16), 20);
	v.setUint32(24, 3);
	v.setUint16(28, 1);
	v.setUint16(30, 1);
	out.set(header.subarray(24, 65), 32);
	out.set(header.subarray(65, 81), 73);
	out.set(header.subarray(81, 113), 89);
	if (codec === 'dds') out.set(header.subarray(113, 121), 121);
	else out[121] = 2;
	out.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', encoded)),
		129,
	);
	out.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', preview)),
		161,
	);
	out[193] = codec === 'dds' ? 64 : 8;
	out[198] = version.length;
	out.set(text.encode(version), 199);
	out[265] = 4;
	out[266] = 1;
	out.set(header.subarray(128, 160), 288);
	out.set(header.subarray(160, 224), 320);
	out.set(header.subarray(224, 256), 384);
	out.set(header.subarray(256, 320), 416);
	out.set(header.subarray(320, 448), 480);
	out.set(encoded, 608);
	out.set(preview, 611);
	return out;
}
for (const codec of ['dds', 'msp']) {
	const api = codec === 'dds' ? dds : msp,
		old = codec === 'dds' ? oldDds : oldMsp,
		title = codec === 'dds' ? 'Dds' : 'Msp';
	test(`${codec}7 exact appended context and every response byte bound;4 remains closed`, async () => {
		const captured = await api[`capture${title}`](
			new Uint8Array([5, 1, 2]),
			owner,
			plan(codec),
			nonce,
		);
		assert.equal(captured.header.length, 448);
		assert.equal(new DataView(captured.header.buffer).getUint16(8), 7);
		const out = await response(codec, captured.header);
		const decoded = await api[`decode${title}Response`](
			out,
			captured,
			owner,
			version,
		);
		assert.deepEqual([...decoded.preview], [0, 0, 0, 255]);
		for (let at = 416; at < 608; at++) {
			const bad = out.slice();
			bad[at] ^= 1;
			await assert.rejects(() =>
				api[`decode${title}Response`](bad, captured, owner, version),
			);
		}
		const oldCaptured = await old[`capture${title}`](
			new Uint8Array([1]),
			owner,
			plan(codec),
			nonce,
		);
		await assert.rejects(() =>
			old[`decode${title}Response`](out, oldCaptured, owner, version),
		);
	});
	test(`${codec}7 accounting fields reject overflow/reserved data and ignore hostile iterators`, () => {
		for (const at of [256 + 12, 256 + 48]) {
			const p = plan(codec);
			p.spatialContext[at - 256] = 1;
			assert.throws(() => api[`freeze${title}Plan`](p));
		}
		for (const at of [24, 32, 40]) {
			const p = plan(codec);
			new DataView(p.spatialContext.buffer).setBigUint64(
				at,
				0xffffffffffffffffn,
			);
			assert.throws(() => api[`freeze${title}Plan`](p));
		}
		const p = plan(codec);
		p.spatialContext[Symbol.iterator] = () => {
			throw Error('iterator must never run');
		};
		const frozen = api[`freeze${title}Plan`](p);
		assert.equal(frozen.spatialContext.length, 64);
		p.spatialContext.fill(0);
		assert.equal(frozen.spatialContext[1], 6);
	});
	test(`${codec}7 reserves before exact finalization; changed bake authority cannot create job`, async () => {
		const service = await import(
			`../src/lib/codec-worker.js?spatial=${codec}-${crypto.randomUUID()}`
		);
		let finalized = 0,
			created = 0,
			stopped = false;
		const options = {
			plan: plan(codec),
			owner,
			nonce,
			currentOwner: () => owner,
			applicationVersion: version,
			finalizePlan: (length) => {
				finalized++;
				assert.equal(length, 3);
				assert.ok(service.codecWorkerBudget().reservedBytes > 0);
				return plan(codec);
			},
			createWorker: () => {
				created++;
				return {
					terminate() {
						stopped = true;
					},
					postMessage(data) {
						assert.equal(data.kind, `${codec}-prepared-export`);
						const request = new Uint8Array(data.bytes);
						response(codec, request.subarray(0, 448)).then(
							(bytes) =>
								this.onmessage({
									data: {
										kind: 'result',
										bytes: bytes.buffer,
									},
								}),
						);
					},
				};
			},
		};
		const ticket = service[`admit${title}InWorker`](options);
		assert.equal(finalized, 0);
		const raw = await ticket.run(new Uint8Array([5, 1, 2]));
		assert.ok(service.codecWorkerBudget().reservedBytes > 0);
		raw.release();
		assert.equal(finalized, 1);
		assert.equal(created, 1);
		assert.equal(stopped, true);
		assert.equal(service.codecWorkerBudget().reservedBytes, 0);
		options.finalizePlan = () => ({ ...plan(codec), bakeIntent: 0 });
		const bad = service[`admit${title}InWorker`](options);
		await assert.rejects(
			() => bad.run(new Uint8Array([5, 1, 2])),
			/plan changed/,
		);
		assert.equal(created, 1);
		assert.equal(service.codecWorkerBudget().reservedBytes, 0);
	});
}

for (const codec of ['dds', 'msp']) {
	const title = codec === 'dds' ? 'Dds' : 'Msp';
	test(`${codec}7 accepts an exact late plan and refuses missing or changed witness`, async () => {
		const service = await import(
			`../src/lib/codec-worker.js?late=${codec}-${crypto.randomUUID()}`
		);
		let created = 0;
		const options = {
			owner,
			nonce,
			currentOwner: () => owner,
			applicationVersion: version,
			plan: plan(codec),
			createWorker: () => {
				created++;
				throw Error('not reached');
			},
		};
		assert.throws(
			() =>
				service[`admit${title}InWorker`]({
					...options,
					plan: { ...plan(codec), protocolVersion: 8 },
				}),
			/Unsupported export protocol/,
		);
		assert.equal(service.codecWorkerBudget().reservedBytes, 0);
		const missing = service[`admit${title}InWorker`](options);
		assert.ok(service.codecWorkerBudget().reservedBytes > 0);
		await assert.rejects(
			() => missing.run(new Uint8Array([5, 1, 2])),
			/profile/,
		);
		assert.equal(created, 0);
		assert.equal(service.codecWorkerBudget().reservedBytes, 0);
		const changed = service[`admit${title}InWorker`](options);
		await assert.rejects(
			() =>
				changed.run(new Uint8Array([5, 1, 2]), {
					...plan(codec),
					bakeIntent: 0,
				}),
			/plan changed/,
		);
		assert.equal(created, 0);
		assert.equal(service.codecWorkerBudget().reservedBytes, 0);
		options.createWorker = () => {
			created++;
			return {
				terminate() {},
				postMessage(data) {
					const request = new Uint8Array(data.bytes);
					response(codec, request.subarray(0, 448)).then((bytes) =>
						this.onmessage({
							data: { kind: 'result', bytes: bytes.buffer },
						}),
					);
				},
			};
		};
		const accepted = service[`admit${title}InWorker`](options);
		const raw = await accepted.run(new Uint8Array([5, 1, 2]), plan(codec));
		assert.equal(created, 1);
		assert.ok(service.codecWorkerBudget().reservedBytes > 0);
		raw.release();
		assert.equal(service.codecWorkerBudget().reservedBytes, 0);
	});
}

test('actual installed service forwards exact late Main plan through its run wrapper', async () => {
	const { createCodecService } = await import('../src/lib/codec-bridge.js');
	const previous = globalThis.Worker;
	try {
		for (const codec of ['dds', 'msp']) {
			let created = 0;
			globalThis.Worker = class {
				constructor() {
					created++;
				}
				terminate() {}
				postMessage(data) {
					assert.equal(data.kind, `${codec}-prepared-export`);
					const request = new Uint8Array(data.bytes);
					response(codec, request.subarray(0, 448)).then((bytes) =>
						this.onmessage({
							data: { kind: 'result', bytes: bytes.buffer },
						}),
					);
				}
			};
			const service = createCodecService(version),
				title = codec === 'dds' ? 'Dds' : 'Msp';
			service.setOwner(owner);
			assert.equal(service[`${codec}ExportProtocolVersion`](), 4);
			assert.equal(service[`${codec}SpatialExportProtocolVersion`](), 5);
			assert.equal(service[`${codec}CatalogExportProtocolVersion`](), 6);
			assert.equal(service[`${codec}PreparedExportProtocolVersion`](), 7);
			const initial = plan(codec);
			delete initial.spatialContext;
			const ticket = service[`begin${title}Export`](
				owner,
				initial,
				nonce,
			);
			assert.equal(created, 0);
			const raw = await ticket.run(
				new Uint8Array([5, 1, 2]),
				plan(codec),
			);
			assert.equal(created, 1);
			assert.equal(raw.requestHeader.length, 448);
			raw.release();
			assert.equal(service.budget().reservedBytes, 0);
		}
	} finally {
		if (previous === undefined) delete globalThis.Worker;
		else globalThis.Worker = previous;
	}
});

for (const codec of ['dds', 'msp']) {
	const api = codec === 'dds' ? dds : msp,
		title = codec === 'dds' ? 'Dds' : 'Msp';
	test(`${codec}7 generic counts allow enabled direct filters and reject every malformed field family`, () => {
		const direct = plan(codec),
			v = new DataView(direct.programContext.buffer);
		v.setUint32(16, 0);
		for (const at of [24, 32, 40, 48]) v.setBigUint64(at, 0n);
		v.setBigUint64(56, 100n);
		assert.equal(
			api[`freeze${title}Plan`](direct).programContext.length,
			128,
		);
		const mutations = [
			(p) => new DataView(p.programContext.buffer).setUint16(0, 2),
			(p) => new DataView(p.programContext.buffer).setUint16(2, 2),
			(p) => new DataView(p.programContext.buffer).setUint32(20, 1),
			(p) => new DataView(p.programContext.buffer).setUint32(4, 257),
			(p) => new DataView(p.programContext.buffer).setUint32(8, 2049),
			(p) => new DataView(p.programContext.buffer).setUint32(12, 2),
			(p) => new DataView(p.programContext.buffer).setUint32(16, 2),
			(p) => new DataView(p.programContext.buffer).setBigUint64(24, 0n),
			(p) => new DataView(p.programContext.buffer).setBigUint64(32, 255n),
			(p) => new DataView(p.programContext.buffer).setBigUint64(40, 263n),
			(p) => new DataView(p.programContext.buffer).setBigUint64(48, 265n),
			(p) =>
				new DataView(p.programContext.buffer).setBigUint64(56, 1101n),
			(p) =>
				new DataView(p.programContext.buffer).setBigUint64(
					40,
					67108865n,
				),
			(p) => new DataView(p.spatialContext.buffer).setUint16(2, 1),
		];
		for (const change of mutations) {
			const p = plan(codec);
			change(p);
			assert.throws(() => api[`freeze${title}Plan`](p));
		}
		const hostile = plan(codec);
		hostile.programContext[Symbol.iterator] = () => {
			throw Error('untrusted iterator');
		};
		const frozen = api[`freeze${title}Plan`](hostile);
		hostile.programContext.fill(0);
		assert.equal(frozen.programContext[1], 1);
		assert.equal(frozen.programContext[64], 31);
	});
}

for (const codec of ['dds', 'msp']) {
	const title = codec === 'dds' ? 'Dds' : 'Msp';
	test(`${codec}7 stale owner after dispatch retires worker, rejects late result and quarantines uncertainty`, async () => {
		const service = await import(
			`../src/lib/codec-worker.js?stale7=${codec}-${crypto.randomUUID()}`
		);
		const abort = new AbortController();
		let current = owner,
			worker,
			posted;
		const ready = new Promise((resolve) => {
			posted = resolve;
		});
		const options = {
			plan: plan(codec),
			owner,
			nonce,
			applicationVersion: version,
			currentOwner: () => current,
			signal: abort.signal,
			createWorker: () =>
				(worker = {
					terminate() {
						this.terminated = true;
					},
					postMessage(data) {
						this.header = new Uint8Array(data.bytes).slice(0, 448);
						posted();
					},
				}),
		};
		const ticket = service[`admit${title}InWorker`](options);
		const pending = ticket.run(new Uint8Array([5, 1, 2]), plan(codec));
		await ready;
		current = { ...owner, epoch: owner.epoch + 1n };
		abort.abort({ code: 'ConflictStaleBase' });
		await assert.rejects(
			() => pending,
			(error) => error.code === 'ConflictStaleBase',
		);
		assert.equal(worker.terminated, true);
		const charged = service.codecWorkerBudget().reservedBytes;
		assert.ok(charged > 0);
		assert.equal(service.codecWorkerBudget().quarantined, true);
		const late = await response(codec, worker.header);
		await worker.onmessage({
			data: { kind: 'result', bytes: late.buffer },
		});
		assert.equal(service.codecWorkerBudget().reservedBytes, charged);
		assert.equal(service.codecWorkerBudget().quarantined, true);
	});
}
