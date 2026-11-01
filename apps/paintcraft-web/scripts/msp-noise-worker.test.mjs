// Authored descriptive JS tests; no valid-ABI/Rust execution authority implied.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { webcrypto, createHash } from 'node:crypto';
import test from 'node:test';
import {
	captureMsp,
	encodeMspRequest,
	freezeMspPlan,
	freezeInitialMspPlan,
	decodeMspResponse,
	isCompletedMspRefusal,
} from '../src/lib/msp-noise-worker-protocol.js';
import {
	admitMspInWorker,
	codecWorkerBudget,
} from '../src/lib/codec-worker.js';
import { freezeMspPlan as legacy } from '../src/lib/msp-worker-protocol.js';
const vector = JSON.parse(
	readFileSync(
		new URL('./fixtures/msp-noise-wire/vector.json', import.meta.url),
	),
);
const bytes = (s) => new Uint8Array(Buffer.from(s, 'hex'));
const owner = {
	session: '00000000-0000-0000-0000-000000000001',
	state: '00000000-0000-0000-0000-000000000002',
	epoch: 1n,
	frontend: 3,
};
const nonce = '00000000-0000-0000-0000-000000000003';
function plan(version) {
	const metadata = new Uint8Array(64);
	metadata[49] = 2;
	metadata[50] = 1;
	return {
		protocolVersion: 8,
		version,
		width: 1,
		height: 1,
		sourceFlags: 4,
		bakeIntent: 1,
		filterDigest: new Uint8Array(32),
		metadataContext: metadata,
		maskContext: new Uint8Array(32),
		noiseWorkload: bytes(vector.workload),
		noiseRegionContext: bytes(vector.region),
		noiseProgramContext: bytes(vector.program),
	};
}
const digest = (b) => createHash('sha256').update(b).digest();
// Deliberately descriptive response only. Rust file/quote validation remains
// mandatory: these tiny arbitrary encoded bytes are not a real MSP image.
function response(captured, status = 0) {
	const encoded = status ? new Uint8Array() : new Uint8Array([1, 2]),
		preview = status
			? new Uint8Array()
			: new Uint8Array([255, 255, 255, 255]);
	const b = new Uint8Array(864 + encoded.length + preview.length),
		v = new DataView(b.buffer),
		h = captured.header;
	b.set(new TextEncoder().encode('PCMSPR08'));
	v.setUint16(8, 8);
	b[10] = status;
	v.setUint32(12, 1);
	v.setUint32(16, 1);
	v.setUint32(20, captured.source.length);
	v.setUint32(24, encoded.length);
	v.setUint16(28, status ? 0 : 1);
	v.setUint16(30, status ? 0 : 1);
	for (const [dest, src, n] of [
		[32, 24, 41],
		[73, 65, 16],
		[89, 81, 32],
		[121, 113, 8],
		[288, 128, 32],
		[320, 160, 64],
		[384, 224, 32],
		[416, 256, 128],
		[544, 384, 256],
		[800, 640, 64],
	])
		b.set(h.subarray(src, src + n), dest);
	b.set(digest(encoded), 129);
	b.set(digest(preview), 161);
	b[193] = status ? 0 : 8;
	b[198] = 5;
	b.set(new TextEncoder().encode('0.1.1'), 199);
	b[265] = 4;
	b[266] = 1;
	b.set(encoded, 864);
	b.set(preview, 864 + encoded.length);
	return b;
}
test('MSP1/2 independent complete704-byte literal requests and detached copies', async () => {
	for (const version of [1, 2]) {
		const p = plan(version),
			source = bytes(vector.source),
			captured = await captureMsp(source, owner, p, nonce, webcrypto);
		assert.deepEqual(
			encodeMspRequest(captured),
			bytes(vector.requests[String(version)]),
		);
		assert.equal(captured.header[113], version);
		assert.equal(captured.header[121], 4);
		assert.equal(captured.header[114], 0);
		assert.deepEqual(
			captured.header.subarray(256, 384),
			bytes(vector.region),
		);
		assert.deepEqual(
			captured.header.subarray(384, 640),
			bytes(vector.program),
		);
		assert.deepEqual(
			captured.header.subarray(640, 704),
			bytes(vector.workload),
		);
		p.noiseProgramContext.fill(9);
		p.noiseRegionContext.fill(9);
		p.noiseWorkload.fill(9);
		source.fill(9);
		assert.deepEqual(
			encodeMspRequest(captured),
			bytes(vector.requests[String(version)]),
		);
	}
});
test('MSP8 initial authority admits bothversions without finalized contexts, legacy remains2only', () => {
	for (const version of [1, 2]) {
		const p = plan(version);
		delete p.noiseRegionContext;
		delete p.noiseProgramContext;
		assert.equal(freezeInitialMspPlan(p).version, version);
		assert.throws(() => freezeMspPlan(p));
	}
	assert.throws(() => legacy(plan(1)));
	for (const version of [0, 3, 1.5, '1'])
		assert.throws(() => freezeInitialMspPlan({ ...plan(1), version }));
});
test('MSP8 defensive source copy precedes new workload SHA await/detachment', async () => {
	let unblock;
	const gate = new Promise((ok) => {
		unblock = ok;
	});
	const fake = {
		subtle: {
			async digest(...args) {
				await gate;
				return webcrypto.subtle.digest(...args);
			},
		},
	};
	const original = bytes(vector.source),
		before = original.slice(),
		promise = captureMsp(original, owner, plan(1), nonce, fake);
	structuredClone(original.buffer, { transfer: [original.buffer] });
	unblock();
	const captured = await promise;
	assert.deepEqual(captured.source, before);
	assert.equal(
		new DataView(captured.header.buffer).getUint32(12),
		before.length,
	);
});
test('MSP8 version/settings/contexts are bound on response and tampering refuses', async () => {
	for (const version of [1, 2]) {
		const captured = await captureMsp(
				bytes(vector.source),
				owner,
				plan(version),
				nonce,
				webcrypto,
			),
			b = response(captured);
		assert.equal(
			(await decodeMspResponse(b, captured, owner, '0.1.1')).encoded
				.length,
			2,
		);
		for (const at of [8, 121, 122, 194, 265, 266, 416, 544, 800, 832]) {
			const bad = b.slice();
			bad[at] ^= 1;
			await assert.rejects(
				decodeMspResponse(bad, captured, owner, '0.1.1'),
				(e) => e.code === 'CodecFailed',
			);
		}
	}
});
test('completed refusal proof is private per actual captured request', async () => {
	const a = await captureMsp(
			bytes(vector.source),
			owner,
			plan(1),
			nonce,
			webcrypto,
		),
		b = await captureMsp(
			bytes(vector.source),
			owner,
			plan(1),
			nonce,
			webcrypto,
		);
	let refusal;
	try {
		await decodeMspResponse(response(a, 3), a, owner, '0.1.1');
	} catch (e) {
		refusal = e;
	}
	assert.equal(refusal.code, 'ResourceLimit');
	assert.equal(isCompletedMspRefusal(refusal, a), true);
	assert.equal(isCompletedMspRefusal(refusal, b), false);
	assert.equal(isCompletedMspRefusal({ code: 'ResourceLimit' }, a), false);
});
test('new service MSP8 initial version authority/final plan staleness refuses before Worker', async () => {
	for (const version of [1, 2]) {
		let created = 0;
		const p = plan(version),
			admitted = admitMspInWorker({
				owner,
				plan: p,
				nonce,
				applicationVersion: '0.1.1',
				currentOwner: () => owner,
				createWorker: () => {
					created++;
					throw Error('must not construct');
				},
			});
		assert.equal(codecWorkerBudget().reservedBytes, 512 * 1048576);
		await assert.rejects(
			admitted.run(bytes(vector.source), plan(version === 1 ? 2 : 1)),
			(e) => e.code === 'ConflictStaleBase',
		);
		assert.equal(created, 0);
		assert.equal(codecWorkerBudget().reservedBytes, 0);
	}
});
test('MSP8 dimensions/family/payload/control limits remain closed', () => {
	for (const p of [
		{ ...plan(1), width: 65536 },
		{ ...plan(1), height: 0 },
		{ ...plan(1), width: 4096, height: 4096 },
	])
		assert.throws(() => freezeMspPlan(p));
	const family = plan(1);
	family.metadataContext[49] = 1;
	assert.throws(() => freezeMspPlan(family));
	for (const at of [0, 2, 4, 6, 112]) {
		const p = plan(1);
		p.noiseRegionContext[at] ^= 1;
		assert.throws(() => freezeMspPlan(p));
	}
	const p = plan(1);
	new DataView(p.noiseRegionContext.buffer).setBigUint64(
		64,
		160n * 1048576n + 1n,
	);
	assert.throws(() => freezeMspPlan(p));
});

test('MSP8 service forwards exactversion1/2 request and retains lease until callerrelease', async () => {
	for (const version of [1, 2]) {
		let sent,
			terminated = 0;
		const p = plan(version),
			admitted = admitMspInWorker({
				owner,
				plan: p,
				nonce,
				applicationVersion: '0.1.1',
				currentOwner: () => owner,
				createWorker: () => ({
					terminate() {
						terminated++;
					},
					postMessage(message) {
						sent = message;
						const request = new Uint8Array(message.bytes);
						const framed = {
							header: request.subarray(0, 704),
							source: request.subarray(704),
							plan: p,
						};
						queueMicrotask(() =>
							this.onmessage?.({
								data: {
									kind: 'result',
									bytes: response(framed).buffer,
								},
							}),
						);
					},
				}),
			});
		const lease = await admitted.run(bytes(vector.source), p);
		try {
			assert.equal(sent.kind, 'msp-noise-export');
			assert.deepEqual(
				lease.requestHeader,
				bytes(vector.requests[String(version)]).subarray(0, 704),
			);
			assert.equal(lease.requiresRustValidation, true);
			assert.equal(terminated, 1);
			assert.equal(codecWorkerBudget().reservedBytes, 512 * 1048576);
		} finally {
			lease.release();
		}
		assert.equal(codecWorkerBudget().reservedBytes, 0);
	}
});
