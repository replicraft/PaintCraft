// Worker dispatcher mocks, not real Wasm/source/pixel/counter qualification.
import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';
import { encodeProgram } from '../src/lib/filter-preview-program-protocol.js';
import { sha } from '../src/lib/filter-preview-protocol.js';
import {
	deferred,
	harness,
	visualOutput,
} from './fixtures/layer-appearance-runtime.mjs';

globalThis.crypto ??= webcrypto;
async function close(h) {
	if (!h.events.some((e) => e.kind === 'closed')) await h.send('close');
}

test('mocked API seam ordering: ready then union HELLO before one source OPEN', async (t) => {
	const h = await harness();
	t.after(() => close(h));
	await h.send('open', { source: new ArrayBuffer(1), plan: h.plan });
	assert.equal(
		h.calls.some((c) => Array.isArray(c) && c[0] === 'open'),
		false,
	);
	await h.start();
	assert.deepEqual(
		h.events
			.filter((e) =>
				['runtime-ready', 'capabilities', 'ready'].includes(e.kind),
			)
			.map((e) => e.kind),
		['runtime-ready', 'capabilities', 'ready'],
	);
	assert.equal(
		h.events.find((e) => e.kind === 'runtime-ready').controlQuote,
		h.runtimeFacts.controlQuoteBytes,
	);
	assert.deepEqual(h.events.find((e) => e.kind === 'ready').binding, h.plan);
	assert.equal(
		h.calls.filter((c) => Array.isArray(c) && c[0] === 'open').length,
		1,
	);
});
test('Source7 refuses shared/view source buffers and a changed sealed admission', async (t) => {
	const h = await harness();
	t.after(() => close(h));
	await h.send('runtime-ready', { noise: false });
	await h.send('hello', h.hello);
	await h.send('open', { source: new Uint8Array(2), plan: h.plan });
	assert.equal(h.events.at(-1).kind, 'error');
	assert.equal(
		h.calls.some((c) => Array.isArray(c) && c[0] === 'open'),
		false,
	);
	if (typeof SharedArrayBuffer === 'function') {
		await h.send('open', {
			source: new SharedArrayBuffer(2),
			plan: h.plan,
		});
		assert.equal(h.events.at(-1).kind, 'error');
	}
	const changed = structuredClone(h.plan);
	changed.admission.baseWorkLimit = '100';
	changed.admission.totalWorkLimit = '100';
	await h.send('open', { source: new ArrayBuffer(2), plan: changed });
	assert.equal(h.events.at(-1).kind, 'error');
	assert.equal(
		h.calls.some((c) => Array.isArray(c) && c[0] === 'open'),
		false,
	);
});
test('startup close joins actual initialization and suppresses late ready before ACK', async () => {
	const d = deferred(),
		h = await harness({ initialize: () => d.promise });
	const ready = h.send('runtime-ready', { noise: false });
	const joined = h.send('close');
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(
		h.events.some((e) => e.kind === 'closed'),
		false,
	);
	d.resolve({ memory: { buffer: new ArrayBuffer(8) } });
	await Promise.all([ready, joined]);
	assert.deepEqual(
		h.events.map((e) => e.kind),
		['closed'],
	);
	assert.equal(h.calls.includes('port1-close'), true);
	assert.equal(h.calls.includes('port2-close'), true);
});
test('one MessageChannel mock FFI step per task, with transferable exact frame extent', async (t) => {
	const h = await harness({ steps: 3 });
	t.after(() => close(h));
	await h.start();
	const r = await h.register(1);
	await h.send('update', { request: r });
	await h.until(() =>
		h.calls.some((c) => Array.isArray(c) && c[0] === 'step'),
	);
	assert.equal(
		h.calls.filter((c) => Array.isArray(c) && c[0] === 'step').length,
		1,
	);
	assert.equal(
		h.events.some((e) => e.kind === 'frame'),
		false,
	);
	await h.until(() => h.events.some((e) => e.kind === 'frame'));
	const result = h.events.find((e) => e.kind === 'frame');
	assert.equal(result.pixels.byteLength, result.width * result.height * 4);
	assert.equal(result.generation, '1');
	assert.equal(
		h.calls.filter((c) => Array.isArray(c) && c[0] === 'step').length,
		3,
	);
});
test('bounded coalescing retires old verifying/desired generations without preparing them', async (t) => {
	const h = await harness();
	t.after(() => close(h));
	await h.start();
	const requests = await Promise.all([
		h.register(1),
		h.register(2),
		h.register(3),
	]);
	await Promise.all(requests.map((request) => h.send('update', { request })));
	await h.until(() =>
		h.events.some((e) => e.kind === 'frame' && e.generation === '3'),
	);
	assert.deepEqual(
		h.calls
			.filter((c) => Array.isArray(c) && c[0] === 'prepare')
			.map((c) => c[1]),
		['3'],
	);
	assert.deepEqual(
		h.events.filter((e) => e.kind === 'retired').map((e) => e.generation),
		['1', '2'],
	);
});
test('late cancellation cut never calls Rust cancel or retire on a newer accepted generation', async (t) => {
	const h = await harness({ steps: 4 });
	t.after(() => close(h));
	await h.start();
	await h.send('update', { request: await h.register(1) });
	await h.until(() =>
		h.calls.some(
			(c) => Array.isArray(c) && c[0] === 'step' && c[1] === '1',
		),
	);
	await h.send('update', { request: await h.register(2) });
	await h.until(() =>
		h.calls.some(
			(c) => Array.isArray(c) && c[0] === 'prepare' && c[1] === '2',
		),
	);
	const before = h.calls.filter(
		(c) => c === 'retire' || (Array.isArray(c) && c[0] === 'cancel'),
	).length;
	await h.send('cancel', { generation: '1' });
	assert.equal(
		h.calls.filter(
			(c) => c === 'retire' || (Array.isArray(c) && c[0] === 'cancel'),
		).length,
		before,
	);
	await h.until(() =>
		h.events.some((e) => e.kind === 'frame' && e.generation === '2'),
	);
	assert.deepEqual(
		h.calls
			.filter((c) => Array.isArray(c) && c[0] === 'cancel')
			.map((c) => c[1]),
		['1'],
	);
});
test('BEGIN inputs detach before async validation and quote-only predictions remain nonzero', async (t) => {
	const h = await harness();
	t.after(() => close(h));
	await h.start();
	const r = await h.register(1, false);
	await h.send('update', { request: r });
	r.owner.state = 'ffffffff-ffff-ffff-ffff-ffffffffffff';
	r.generation = '99';
	await h.until(() => h.events.some((e) => e.kind === 'prepared'));
	const result = h.events.find((e) => e.kind === 'prepared');
	assert.equal(result.generation, '1');
	assert.equal(result.owner.state, h.plan.owner.state);
	assert.notEqual(result.workQuote.baseWork, '0');
	assert.equal(Object.hasOwn(result, 'pixels'), false);
	assert.equal(
		h.calls.some((c) => Array.isArray(c) && c[0] === 'step'),
		false,
	);
});
test('only actual typed Rust string errors establish completed, not forged JS flags', async () => {
	for (const [error, completed] of [
		[
			JSON.stringify({
				code: 'resource.limit',
				message: 'mock Rust limit',
			}),
			true,
		],
		[
			Object.assign(new Error('mock trap'), {
				code: 'resource.limit',
				completed: true,
			}),
			false,
		],
	]) {
		const h = await harness({ prepareError: error });
		try {
			await h.start();
			await h.send('update', { request: await h.register(1) });
			await h.until(() => h.events.some((e) => e.kind === 'error'));
			assert.equal(
				h.events.find((e) => e.kind === 'error').completed,
				completed,
			);
		} finally {
			await close(h);
		}
	}
});
test('visual copy rejects foreign download, then uses prepareCopy/encodedFrame seam', async (t) => {
	const options = {},
		h = await harness(options);
	t.after(() => close(h));
	await h.start();
	const output = await visualOutput(),
		request = await h.register(2);
	options.copySha = await sha(
		new TextEncoder().encode(encodeProgram(output, 4096)),
	);
	await h.send('copy-prepare', {
		request: await h.register(1),
		output: { ...output, purpose: 'download' },
	});
	assert.equal(h.events.at(-1).kind, 'error');
	assert.equal(
		h.calls.some((c) => Array.isArray(c) && c[0] === 'prepare'),
		false,
	);
	await h.send('copy-prepare', { request, output });
	const admitted = h.events.find((e) => e.kind === 'copy-admitted');
	assert.ok(admitted);
	await h.send('copy-input', {
		outputSha256: admitted.outputSha256,
		mask: new ArrayBuffer(0),
	});
	await h.until(() => h.events.some((e) => e.kind === 'encoded-copy'));
	const result = h.events.find((e) => e.kind === 'encoded-copy');
	assert.equal(result.mime, 'image/png');
	assert.equal(await sha(result.bytes), result.bytesSha256);
	assert.equal(result.binding.protocol, 7);
	assert.equal(result.outputSha256, options.copySha);
});
test('cancel suppresses held encoded SHA and close waits digest then real free before ACK', async () => {
	const d = deferred(),
		options = { digestEncoded: () => d.promise },
		h = await harness(options);
	await h.start();
	const output = await visualOutput();
	options.copySha = await sha(
		new TextEncoder().encode(encodeProgram(output, 4096)),
	);
	await h.send('copy-prepare', { request: await h.register(1), output });
	const admitted = h.events.find((e) => e.kind === 'copy-admitted');
	await h.send('copy-input', {
		outputSha256: admitted.outputSha256,
		mask: new ArrayBuffer(0),
	});
	await h.until(() =>
		h.calls.some((c) => Array.isArray(c) && c[0] === 'encoded'),
	);
	await h.send('cancel', { generation: '1' });
	const joined = h.send('close');
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(h.calls.includes('free'), false);
	assert.equal(
		h.events.some((e) => e.kind === 'closed'),
		false,
	);
	d.resolve('aa'.repeat(32));
	await joined;
	assert.equal(
		h.events.some((e) => e.kind === 'encoded-copy'),
		false,
	);
	assert.equal(h.calls.filter((c) => c === 'free').length, 1);
	const freeAt = h.calls.indexOf('free'),
		ackAt = h.calls.findIndex(
			(c) => Array.isArray(c) && c[0] === 'post' && c[1] === 'closed',
		);
	assert.ok(freeAt >= 0 && freeAt < ackAt);
});
test('duplicate ids and nonadvancing generation are refused without another Rust preparation', async (t) => {
	const h = await harness();
	t.after(() => close(h));
	await h.start();
	const request = await h.register(1, false);
	await h.send('update', { request });
	await h.until(() => h.events.some((e) => e.kind === 'prepared'));
	await h.send('update', { request });
	assert.equal(h.events.at(-1).kind, 'error');
	const repliesBeforeDuplicate = h.events.length;
	await h.runtime.receive({
		data: { kind: 'cancel', id: 1, generation: '1' },
	});
	assert.equal(h.events.length, repliesBeforeDuplicate + 1);
	assert.equal(h.events.at(-1).kind, 'error');
	assert.equal(h.events.at(-1).id, 1);
	assert.equal(
		h.calls.filter((c) => Array.isArray(c) && c[0] === 'prepare').length,
		1,
	);
});

test('malformed actual frame extent fails privately rather than posting pixels', async (t) => {
	const h = await harness({ frameBytes: [] });
	t.after(() => close(h));
	await h.start();
	await h.send('update', { request: await h.register(1) });
	await h.until(() => h.events.some((e) => e.kind === 'error'));
	assert.equal(
		h.events.some((e) => e.kind === 'frame'),
		false,
	);
	assert.equal(h.events.find((e) => e.kind === 'error').completed, false);
	assert.equal(
		h.calls.some(
			(c) => Array.isArray(c) && c[0] === 'cancel' && c[1] === '1',
		),
		true,
	);
});

test('bad async union hash cannot retire prior accepted Rust work', async (t) => {
	const h = await harness({ steps: 4 });
	t.after(() => close(h));
	await h.start();
	await h.send('update', { request: await h.register(1) });
	await h.until(() =>
		h.calls.some((c) => Array.isArray(c) && c[0] === 'step'),
	);
	const bad = await h.register(2);
	bad.unionRequirements.capabilitySha256 = 'ff'.repeat(32);
	await h.send('update', { request: bad });
	await h.until(() => h.events.some((e) => e.kind === 'error'));
	assert.equal(
		h.calls.some((c) => Array.isArray(c) && c[0] === 'cancel'),
		false,
	);
	assert.equal(h.calls.includes('retire'), false);
	await h.until(() =>
		h.events.some((e) => e.kind === 'frame' && e.generation === '1'),
	);
});

// Metadata/FFI mocks only: proves dispatcher does not cancel genuine delivered success.
test('same-view next generation preserves completed frame and encoded-copy success', async (t) => {
	for (const copy of [false, true]) {
		const options = {},
			h = await harness(options);
		t.after(() => close(h));
		await h.start();
		if (copy) {
			const output = await visualOutput();
			options.copySha = await sha(
				new TextEncoder().encode(encodeProgram(output, 4096)),
			);
			await h.send('copy-prepare', {
				request: await h.register(1),
				output,
			});
			const admitted = h.events.find((e) => e.kind === 'copy-admitted');
			await h.send('copy-input', {
				outputSha256: admitted.outputSha256,
				mask: new ArrayBuffer(0),
			});
			await h.until(() =>
				h.events.some((e) => e.kind === 'encoded-copy'),
			);
		} else {
			await h.send('update', { request: await h.register(1) });
			await h.until(() =>
				h.events.some(
					(e) => e.kind === 'frame' && e.generation === '1',
				),
			);
		}
		await h.send('update', { request: await h.register(2) });
		await h.until(() =>
			h.events.some((e) => e.kind === 'frame' && e.generation === '2'),
		);
		assert.equal(
			h.calls.some((c) => Array.isArray(c) && c[0] === 'cancel'),
			false,
		);
	}
});

test('failed encoded digest cancels accepted Rust generation before error instead of preserving success', async (t) => {
	const options = {
		digestEncoded: async () => {
			throw new Error('mock digest failure');
		},
	};
	const h = await harness(options);
	t.after(() => close(h));
	await h.start();
	const output = await visualOutput();
	options.copySha = await sha(
		new TextEncoder().encode(encodeProgram(output, 4096)),
	);
	await h.send('copy-prepare', { request: await h.register(1), output });
	const admitted = h.events.find((e) => e.kind === 'copy-admitted');
	await h.send('copy-input', {
		outputSha256: admitted.outputSha256,
		mask: new ArrayBuffer(0),
	});
	await h.until(() => h.events.some((e) => e.kind === 'error'));
	assert.equal(
		h.events.some((e) => e.kind === 'encoded-copy'),
		false,
	);
	assert.equal(
		h.calls.some(
			(c) => Array.isArray(c) && c[0] === 'cancel' && c[1] === '1',
		),
		true,
	);
});
