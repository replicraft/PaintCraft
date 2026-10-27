import test from 'node:test';
import manifest from '../package.json' with { type: 'json' };
import assert from 'node:assert/strict';
import { memoryLimits } from './codec-memory.mjs';
import {
	capture,
	ownerBytes,
	decodeResponse,
	RESPONSE_HEADER,
	LINEAR_MEMORY_BYTES,
} from '../src/lib/codec-worker-protocol.js';
import { decodeInWorker, codecWorkerBudget } from '../src/lib/codec-worker.js';
const owner = {
	session: '00000000-0000-4000-8000-000000000001',
	state: '00000000-0000-4000-8000-000000000002',
	epoch: 17n,
	frontend: 3,
};
function response(r, rgba = [10, 20, 30, 0]) {
	const version = new TextEncoder().encode(manifest.version);
	const bytes = new Uint8Array(
		RESPONSE_HEADER + version.length + rgba.length,
	);
	const view = new DataView(bytes.buffer);
	bytes.set(new TextEncoder().encode('PCPIX002'));
	view.setUint16(8, 2);
	bytes[11] = 2;
	view.setUint32(12, 1);
	view.setUint32(16, 1);
	view.setUint16(21, 1);
	view.setUint16(23, 13);
	bytes[27] = 1;
	view.setUint32(28, rgba.length);
	bytes[38] = version.length;
	bytes.set(r.header.subarray(23, 64), 47);
	bytes.set(r.header.subarray(64, 80), 88);
	bytes.set(r.header.subarray(80, 112), 104);
	bytes.set(version, RESPONSE_HEADER);
	bytes.set(rgba, RESPONSE_HEADER + version.length);
	return bytes;
}
function leb(n) {
	const bytes = [];
	do {
		const low = n & 127;
		n >>>= 7;
		bytes.push(low | (n ? 128 : 0));
	} while (n);
	return bytes;
}
function module(sections) {
	return Uint8Array.from([
		0,
		97,
		115,
		109,
		1,
		0,
		0,
		0,
		...sections.flatMap(([id, payload]) => [
			id,
			...leb(payload.length),
			...payload,
		]),
	]);
}
test('actual Wasm declaration permits exactly one defined unshared bounded memory', () => {
	assert.deepEqual(memoryLimits(module([[5, [1, 1, 1, ...leb(4096)]]])), {
		minimum: 1,
		maximum: 4096,
	});
	for (const bytes of [
		module([[5, [1, 0, 1]]]),
		module([[5, [1, 1, 1, ...leb(4097)]]]),
		module([[5, [2, 1, 1, ...leb(4096), 1, 1, ...leb(4096)]]]),
		module([
			[
				2,
				[1, 3, 101, 110, 118, 6, 109, 101, 109, 111, 114, 121, 2, 0, 1],
			],
			[5, [1, 1, 1, ...leb(4096)]],
		]),
	])
		assert.throws(() => memoryLimits(bytes));
});
test('capture freezes source before digest and protocol checks owner, source and codec', async () => {
	await assert.rejects(capture(new Uint8Array([1]), owner, 3, 1), {
		code: 'CapabilityUnsupported',
	});
	const source = new Uint8Array([1, 2, 3]);
	const pending = capture(source, owner, 2, 1);
	source[0] = 99;
	const r = await pending;
	assert.deepEqual([...r.source], [1, 2, 3]);
	const bytes = response(r);
	assert.deepEqual(
		[...decodeResponse(bytes, r, owner, manifest.version).rgba],
		[10, 20, 30, 0],
	);
	for (const changed of [
		{ ...owner, epoch: 18n },
		{ ...owner, frontend: 1 },
		{ ...owner, state: owner.session },
	])
		assert.throws(
			() => decodeResponse(bytes, r, changed, manifest.version),
			{
				code: 'ConflictStaleBase',
			},
		);
	for (const offset of [11, 27, 39, 47, 88, 104]) {
		const changed = bytes.slice();
		changed[offset] ^= 1;
		assert.throws(() =>
			decodeResponse(changed, r, owner, manifest.version),
		);
	}
	assert.throws(() => decodeResponse(bytes, r, owner, '999.0.0'));
	assert.equal(ownerBytes(owner).length, 41);
});
test('controller keeps full admission until result consumption and terminates worker', async () => {
	let terminated = 0;
	const prepared = await decodeInWorker({
		applicationVersion: manifest.version,
		source: new Uint8Array([1, 2, 3]),
		owner,
		codec: 2,
		pixels: 1,
		currentOwner: () => owner,
		createWorker: () => ({
			terminate() {
				terminated++;
			},
			postMessage({ bytes }) {
				const header = new Uint8Array(bytes).slice(0, 112);
				queueMicrotask(() =>
					this.onmessage({
						data: {
							kind: 'result',
							bytes: response({ header }).buffer,
						},
					}),
				);
			},
		}),
	});
	assert.equal(terminated, 1);
	assert.ok(codecWorkerBudget().reservedBytes > LINEAR_MEMORY_BYTES);
	await assert.rejects(
		() =>
			decodeInWorker({
				applicationVersion: manifest.version,
				source: new Uint8Array([1]),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
			}),
		{ code: 'ResourceLimit' },
	);
	prepared.release();
	prepared.release();
	assert.equal(codecWorkerBudget().reservedBytes, 0);
});
test('postMessage failure quarantines uncertain lifetime and retains admission', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		'../src/lib/codec-worker.js?post-failure'
	);
	let terminated = 0;
	await assert.rejects(
		() =>
			decodeInWorker({
				applicationVersion: manifest.version,
				source: new Uint8Array([1]),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
				createWorker: () => ({
					terminate() {
						terminated++;
					},
					postMessage() {
						throw new Error('closed worker');
					},
				}),
			}),
		/closed worker/,
	);
	assert.equal(terminated, 1);
	assert.ok(codecWorkerBudget().reservedBytes > LINEAR_MEMORY_BYTES);
	assert.equal(codecWorkerBudget().quarantined, true);
});

test('captured Rust host version must match response regardless of JS package version', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		'../src/lib/codec-worker.js?version-mismatch'
	);
	let terminated = 0;
	await assert.rejects(
		() =>
			decodeInWorker({
				applicationVersion: '999.0.0',
				source: new Uint8Array([1]),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
				createWorker: () => ({
					terminate() {
						terminated++;
					},
					postMessage({ bytes }) {
						const header = new Uint8Array(bytes).slice(0, 112);
						queueMicrotask(() =>
							this.onmessage({
								data: {
									kind: 'result',
									bytes: response({ header }).buffer,
								},
							}),
						);
					},
				}),
			}),
		{ code: 'CodecFailed' },
	);
	assert.equal(terminated, 1);
	assert.ok(codecWorkerBudget().reservedBytes > LINEAR_MEMORY_BYTES);
	assert.equal(codecWorkerBudget().quarantined, true);
	await assert.rejects(
		() =>
			decodeInWorker({
				source: new Uint8Array([1]),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
			}),
		{ code: 'InputSchema' },
	);
});

test('cancellation quarantines module and refuses later jobs without pretend join', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		'../src/lib/codec-worker.js?cancel-quarantine'
	);
	const controller = new AbortController();
	let terminated = 0;
	const options = {
		applicationVersion: manifest.version,
		source: new Uint8Array([1]),
		owner,
		codec: 2,
		pixels: 1,
		currentOwner: () => owner,
	};
	await assert.rejects(
		() =>
			decodeInWorker({
				...options,
				signal: controller.signal,
				createWorker: () => ({
					terminate() {
						terminated++;
					},
					postMessage() {
						queueMicrotask(() => controller.abort());
					},
				}),
			}),
		{ code: 'OperationCancelled' },
	);
	assert.equal(terminated, 1);
	assert.equal(codecWorkerBudget().quarantined, true);
	assert.ok(codecWorkerBudget().reservedBytes > LINEAR_MEMORY_BYTES);
	await assert.rejects(() => decodeInWorker(options), {
		code: 'ResourceLimit',
	});
});
test('validated canonical completed refusal releases charge and permits another job', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		'../src/lib/codec-worker.js?completed-refusal'
	);
	const options = {
		applicationVersion: manifest.version,
		source: new Uint8Array([1]),
		owner,
		codec: 2,
		pixels: 1,
		currentOwner: () => owner,
	};
	for (let i = 0; i < 2; i++)
		await assert.rejects(
			() =>
				decodeInWorker({
					...options,
					createWorker: () => ({
						terminate() {},
						postMessage({ bytes }) {
							const r = {
								header: new Uint8Array(bytes).slice(0, 112),
							};
							const original = response(r);
							const version = new TextEncoder().encode(
								manifest.version,
							);
							const detail = new TextEncoder().encode(
								'ICC conversion disabled',
							);
							const failure = new Uint8Array(
								RESPONSE_HEADER +
									version.length +
									detail.length,
							);
							failure.set(original.subarray(0, RESPONSE_HEADER));
							failure[10] = 1;
							failure.fill(0, 12, 38);
							new DataView(failure.buffer).setUint16(
								36,
								detail.length,
							);
							failure.set(version, RESPONSE_HEADER);
							failure.set(
								detail,
								RESPONSE_HEADER + version.length,
							);
							queueMicrotask(() =>
								this.onmessage({
									data: {
										kind: 'result',
										bytes: failure.buffer,
									},
								}),
							);
						},
					}),
				}),
			{ code: 'CapabilityUnsupported', codecCompleted: true },
		);
	assert.equal(codecWorkerBudget().reservedBytes, 0);
	assert.equal(codecWorkerBudget().quarantined, false);
});
test('Worker constructor attempt failure quarantines without inventing completion', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		'../src/lib/codec-worker.js?create-failure'
	);
	await assert.rejects(
		() =>
			decodeInWorker({
				applicationVersion: manifest.version,
				source: new Uint8Array([1]),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
				createWorker() {
					throw new Error('no Worker');
				},
			}),
		/no Worker/,
	);
	assert.ok(codecWorkerBudget().reservedBytes > 0);
	assert.equal(codecWorkerBudget().quarantined, true);
});

test('File size is admitted before arrayBuffer and read failure releases pre-worker charge', async () => {
	const { decodeFileInWorker, codecWorkerBudget } = await import(
		`../src/lib/codec-worker.js?file-admission=${Date.now()}`
	);
	let reads = 0;
	class Oversized extends File {
		get size() {
			return 64 * 1024 * 1024 + 1;
		}
		async arrayBuffer() {
			reads++;
			throw new Error('must not read');
		}
	}
	await assert.rejects(
		decodeFileInWorker({
			file: new Oversized([], 'big.jxl'),
			owner,
			codec: 2,
			pixels: 1,
			currentOwner: () => owner,
			applicationVersion: manifest.version,
		}),
		{ code: 'ResourceLimit' },
	);
	assert.equal(reads, 0);
	assert.equal(codecWorkerBudget().reservedBytes, 0);
	class Unreadable extends File {
		get size() {
			return 1;
		}
		async arrayBuffer() {
			reads++;
			throw new Error('unreadable');
		}
	}
	await assert.rejects(
		decodeFileInWorker({
			file: new Unreadable([], 'read.jxl'),
			owner,
			codec: 2,
			pixels: 1,
			currentOwner: () => owner,
			applicationVersion: manifest.version,
		}),
		/unreadable/,
	);
	assert.equal(reads, 1);
	assert.equal(codecWorkerBudget().reservedBytes, 0);
	assert.equal(codecWorkerBudget().quarantined, false);
});

test('hung File read cancels logically without releasing unjoined admission', async () => {
	const { decodeFileInWorker, codecWorkerBudget } = await import(
		`../src/lib/codec-worker.js?hung-file=${Date.now()}`
	);
	class Hung extends File {
		get size() {
			return 1;
		}
		arrayBuffer() {
			return new Promise(() => {});
		}
	}
	const abort = new AbortController();
	const start = performance.now();
	const pending = decodeFileInWorker({
		file: new Hung([], 'hung.jxl'),
		owner,
		codec: 2,
		pixels: 1,
		currentOwner: () => owner,
		applicationVersion: manifest.version,
		signal: abort.signal,
	});
	abort.abort();
	await assert.rejects(pending, { code: 'OperationCancelled' });
	assert.ok(performance.now() - start < 1000);
	assert.ok(codecWorkerBudget().quarantined);
	assert.ok(codecWorkerBudget().reservedBytes > 0);
});
test('hung digest expires logically and quarantines instead of pretending reclamation', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		`../src/lib/codec-worker.js?hung-digest=${Date.now()}`
	);
	const original = crypto.subtle.digest;
	crypto.subtle.digest = () => new Promise(() => {});
	try {
		await assert.rejects(
			decodeInWorker({
				source: new Uint8Array([1]),
				owner,
				codec: 2,
				pixels: 1,
				currentOwner: () => owner,
				applicationVersion: manifest.version,
				deadlineMs: 10,
			}),
			{ code: 'ResourceLimit' },
		);
		assert.ok(codecWorkerBudget().quarantined);
		assert.ok(codecWorkerBudget().reservedBytes > 0);
	} finally {
		crypto.subtle.digest = original;
	}
});

test('bridge checks captured owner instead of rebinding it and invalidates only owner changes', async () => {
	const { createCodecService } = await import('../src/lib/codec-bridge.js');
	const service = createCodecService(manifest.version);
	service.setOwner(owner);
	let reads = 0;
	class Hung extends File {
		get size() {
			return 1;
		}
		arrayBuffer() {
			reads++;
			return new Promise(() => {});
		}
	}
	const file = new Hung([], 'hung.jxl');
	await assert.rejects(service.decodeFile(file, { ...owner, epoch: 18n }), {
		code: 'ConflictStaleBase',
	});
	assert.equal(reads, 0);
	const pending = service.decodeFile(file, owner);
	service.setOwner({ ...owner });
	let completed = false;
	pending.catch(() => {
		completed = true;
	});
	await new Promise((resolve) => setTimeout(resolve, 20));
	assert.equal(completed, false);
	service.setOwner({ ...owner, epoch: 18n });
	await assert.rejects(pending, { code: 'ConflictStaleBase' });
	assert.ok(service.budget().quarantined);
	await assert.rejects(service.decodeFile(file, owner), {
		code: 'ResourceLimit',
	});
});

test('File admission freezes owner before asynchronous file read', async () => {
	const { decodeFileInWorker } = await import(
		`../src/lib/codec-worker.js?frozen-file-owner=${Date.now()}`
	);
	let finish;
	class Delayed extends File {
		get size() {
			return 1;
		}
		arrayBuffer() {
			return new Promise((resolve) => (finish = resolve));
		}
	}
	const captured = { ...owner };
	const pending = decodeFileInWorker({
		file: new Delayed([], 'owner.jxl'),
		owner: captured,
		codec: 2,
		pixels: 1,
		currentOwner: () => owner,
		applicationVersion: manifest.version,
		createWorker: () => ({
			terminate() {},
			postMessage({ bytes }) {
				const header = new Uint8Array(bytes).slice(0, 112);
				assert.deepEqual(header.subarray(23, 64), ownerBytes(owner));
				queueMicrotask(() =>
					this.onmessage({
						data: {
							kind: 'result',
							bytes: response({ header }).buffer,
						},
					}),
				);
			},
		}),
	});
	captured.epoch = 18n;
	finish(new Uint8Array([1]).buffer);
	const prepared = await pending;
	prepared.release();
});

test('completion proof is private to the validated request and cannot be forged or replayed', async () => {
	const { isCompletedRefusal } = await import(
		'../src/lib/codec-worker-protocol.js'
	);
	const previous = await capture(new Uint8Array([1]), owner, 2, 1);
	const original = response(previous);
	const version = new TextEncoder().encode(manifest.version);
	const failure = new Uint8Array(RESPONSE_HEADER + version.length);
	failure.set(original.subarray(0, RESPONSE_HEADER));
	failure[10] = 1;
	failure.fill(0, 12, 38);
	failure[38] = version.length;
	failure.set(version, RESPONSE_HEADER);
	let old;
	try {
		decodeResponse(failure, previous, owner, manifest.version);
	} catch (error) {
		old = error;
	}
	assert.ok(isCompletedRefusal(old, previous));
	const next = await capture(new Uint8Array([1]), owner, 2, 1);
	assert.equal(isCompletedRefusal(old, next), false);
	assert.equal(isCompletedRefusal({ codecCompleted: true }, next), false);
	for (const mode of ['abort', 'postMessage', 'owner', 'constructor']) {
		const { decodeInWorker, codecWorkerBudget } = await import(
			`../src/lib/codec-worker.js?spoof-${mode}=${Date.now()}`
		);
		const abort = new AbortController();
		let notify,
			ownerThrows = false;
		const ready = new Promise((resolve) => (notify = resolve));
		const pending = decodeInWorker({
			source: new Uint8Array([1]),
			owner,
			codec: 2,
			pixels: 1,
			applicationVersion: manifest.version,
			signal: abort.signal,
			currentOwner: () => {
				if (ownerThrows) throw old;
				return owner;
			},
			createWorker: () => {
				if (mode === 'constructor') throw old;
				return {
					terminate() {},
					postMessage({ bytes }) {
						if (mode === 'postMessage') throw old;
						notify();
						if (mode === 'owner') {
							ownerThrows = true;
							queueMicrotask(() =>
								this.onmessage({
									data: {
										kind: 'result',
										bytes: response({
											header: new Uint8Array(bytes).slice(
												0,
												112,
											),
										}).buffer,
									},
								}),
							);
						}
					},
				};
			},
		});
		if (mode === 'abort') {
			await ready;
			abort.abort(
				Object.assign(new Error('stale'), {
					code: 'ConflictStaleBase',
					codecCompleted: true,
				}),
			);
		}
		await assert.rejects(pending);
		assert.ok(codecWorkerBudget().quarantined, mode);
		assert.ok(codecWorkerBudget().reservedBytes > 0, mode);
	}
});

test('Rust nonce and raw response receipt bind handoff; integrity refusal keeps its lease', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		`../src/lib/codec-worker.js?handoff=${Date.now()}`
	);
	const nonce = '00000000-0000-4000-8000-000000000099';
	const prepared = await decodeInWorker({
		source: new Uint8Array([1]),
		owner,
		nonce,
		codec: 2,
		pixels: 1,
		applicationVersion: manifest.version,
		currentOwner: () => owner,
		createWorker: () => ({
			terminate() {},
			postMessage({ bytes }) {
				const header = new Uint8Array(bytes).slice(0, 112);
				queueMicrotask(() =>
					this.onmessage({
						data: {
							kind: 'result',
							bytes: response({ header }).buffer,
						},
					}),
				);
			},
		}),
	});
	assert.equal(
		Buffer.from(prepared.requestHeader.subarray(64, 80)).toString('hex'),
		nonce.replaceAll('-', ''),
	);
	assert.deepEqual(
		prepared.requestHeader.subarray(80, 112),
		prepared.sourceSha256,
	);
	assert.equal(prepared.responseBytes.buffer, prepared.rgba.buffer);
	prepared.quarantine();
	prepared.release();
	assert.ok(codecWorkerBudget().quarantined);
	assert.ok(codecWorkerBudget().reservedBytes > 0);
});

test('legacy validated completion stays synchronous when an abort is queued by the message payload', async () => {
	const { decodeInWorker, codecWorkerBudget } = await import(
		`../src/lib/codec-worker.js?legacy-late-abort=${crypto.randomUUID()}`
	);
	const abort = new AbortController();
	const result = await decodeInWorker({
		source: new Uint8Array([1]),
		owner,
		codec: 2,
		pixels: 1,
		applicationVersion: manifest.version,
		currentOwner: () => owner,
		signal: abort.signal,
		createWorker: () => ({
			terminate() {},
			postMessage(data) {
				const header = new Uint8Array(data.bytes).subarray(0, 112);
				const bytes = response({ header });
				this.onmessage({
					data: {
						kind: 'result',
						get bytes() {
							queueMicrotask(() => abort.abort());
							return bytes.buffer;
						},
					},
				});
			},
		}),
	});
	assert.equal(abort.signal.aborted, true);
	assert.equal(codecWorkerBudget().quarantined, false);
	assert.equal(result.requiresRustValidation, false);
	result.release();
	assert.equal(codecWorkerBudget().reservedBytes, 0);
});
