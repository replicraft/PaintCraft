import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import manifest from '../package.json' with { type: 'json' };
import {
	capture,
	decodeResponse,
	decodeIntegerResponse,
	RESPONSE_HEADER,
} from '../src/lib/codec-worker-protocol.js';

const owner = {
	session: '00000000-0000-4000-8000-000000000001',
	state: '00000000-0000-4000-8000-000000000002',
	epoch: 17n,
	frontend: 3,
};
const source = new Uint8Array([1, 2, 3]);
const rgba = new Uint8Array([17, 29, 91, 0]);
function response(header, depth = 16) {
	const version = new TextEncoder().encode(manifest.version);
	const bytes = new Uint8Array(
		RESPONSE_HEADER + version.length + 96 + rgba.length,
	);
	const view = new DataView(bytes.buffer);
	bytes.set(new TextEncoder().encode('PCPIX002'));
	view.setUint16(8, 2);
	bytes[11] = 9;
	view.setUint32(12, 1);
	view.setUint32(16, 1);
	view.setUint16(21, 1);
	view.setUint16(23, 13);
	bytes[27] = 1;
	view.setUint32(28, rgba.length);
	view.setUint16(36, 96);
	bytes[38] = version.length;
	bytes.set(header.subarray(23, 64), 47);
	bytes.set(header.subarray(64, 80), 88);
	bytes.set(header.subarray(80, 112), 104);
	bytes.set(version, RESPONSE_HEADER);
	const start = RESPONSE_HEADER + version.length;
	bytes.set(new TextEncoder().encode('PCJXLI01'), start);
	view.setUint16(start + 8, 1);
	bytes[start + 10] = depth;
	view.setUint32(start + 12, 1);
	view.setUint32(start + 16, 1);
	bytes.set(header.subarray(80, 112), start + 20);
	bytes.set(createHash('sha256').update(rgba).digest(), start + 52);
	view.setUint16(start + 92, depth === 16 ? 1 : 0);
	bytes.set(rgba, start + 96);
	return bytes;
}
function options(extra = {}) {
	return {
		source,
		owner,
		codec: 9,
		pixels: 1,
		currentOwner: () => owner,
		applicationVersion: manifest.version,
		...extra,
	};
}
function worker(extra = {}) {
	return {
		terminate() {},
		postMessage(request) {
			const header = new Uint8Array(request.bytes).slice(0, 112);
			queueMicrotask(() =>
				this.onmessage({
					data: { kind: 'result', bytes: response(header).buffer },
				}),
			);
		},
		...extra,
	};
}
test('integer8 and16 reports bind precision, source, policy and hidden RGB output digest', async () => {
	const captured = await capture(source, owner, 9, 1);
	for (const depth of [8, 16]) {
		const result = await decodeIntegerResponse(
			response(captured.header, depth),
			captured,
			owner,
			manifest.version,
		);
		assert.deepEqual(result.rgba, rgba);
		assert.equal(result.report.sourceDepth, depth);
		assert.equal(result.report.changedAlpha, 0);
	}
	const good = response(captured.header);
	const report = RESPONSE_HEADER + manifest.version.length;
	for (const offset of [
		0,
		11,
		104,
		report,
		report + 8,
		report + 10,
		report + 11,
		report + 12,
		report + 16,
		report + 20,
		report + 52,
		report + 84,
		report + 88,
		report + 92,
		report + 94,
		good.length - 4,
	]) {
		const changed = good.slice();
		changed[offset] ^= 1;
		await assert.rejects(
			decodeIntegerResponse(changed, captured, owner, manifest.version),
			{ code: offset === 104 ? 'ConflictStaleBase' : 'CodecFailed' },
		);
	}
	const old = await capture(source, owner, 2, 1);
	const wrong = response(old.header);
	wrong[11] = 2;
	assert.throws(() => decodeResponse(wrong, old, owner, manifest.version), {
		code: 'CodecFailed',
	});
});
test('integer Worker admission includes the u16 source plane and persists until explicit consumption', async () => {
	const service = await import('../src/lib/codec-worker.js?integer-budget');
	const result = await service.decodeInWorker(
		options({ createWorker: () => worker() }),
	);
	assert.equal(result.report.sourceDepth, 16);
	assert.equal(result.requiresRustValidation, true);
	assert.ok(
		service.codecWorkerBudget().reservedBytes >=
			256 * 1024 * 1024 + 32 * 1024 * 1024,
	);
	assert.equal(service.codecWorkerBudget().quarantined, false);
	result.release();
	assert.equal(service.codecWorkerBudget().reservedBytes, 0);
});
test('owner change during asynchronous output hashing refuses publication and retains uncertain admission', async () => {
	const service = await import(
		'../src/lib/codec-worker.js?integer-stale-hash'
	);
	let current = owner;
	const original = globalThis.crypto;
	let hashes = 0;
	Object.defineProperty(globalThis, 'crypto', {
		configurable: true,
		value: {
			getRandomValues: original.getRandomValues.bind(original),
			subtle: {
				digest(...args) {
					hashes++;
					if (hashes === 2)
						current = { ...owner, epoch: owner.epoch + 1n };
					return original.subtle.digest(...args);
				},
			},
		},
	});
	try {
		await assert.rejects(
			service.decodeInWorker(
				options({
					currentOwner: () => current,
					createWorker: () => worker(),
				}),
			),
			{ code: 'ConflictStaleBase' },
		);
		assert.equal(hashes, 2);
		assert.equal(service.codecWorkerBudget().quarantined, true);
		assert.ok(service.codecWorkerBudget().reservedBytes > 0);
	} finally {
		Object.defineProperty(globalThis, 'crypto', {
			configurable: true,
			value: original,
		});
	}
});
test('integer bridge selection is explicit and the legacy file API retains codec2', async () => {
	const { createCodecService } = await import(
		'../src/lib/codec-bridge.js?integer-api'
	);
	const service = createCodecService(manifest.version);
	service.setOwner(owner);
	assert.equal(typeof service.beginDecodeIntegerFile, 'function');
	const unused = service.beginDecodeIntegerFile(
		new File([source], 'integer.jxl'),
		owner,
	);
	assert.throws(
		() => service.beginDecodeFile(new File([source], 'legacy.jxl'), owner),
		{ code: 'ResourceLimit' },
	);
	unused.release();
	const legacy = service.beginDecodeFile(
		new File([source], 'legacy.jxl'),
		owner,
	);
	legacy.release();
});
