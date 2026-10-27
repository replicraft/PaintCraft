import assert from 'node:assert/strict';
import test from 'node:test';
import { downloadCopyTracked } from '../src/lib/download.js';

function environment(t, options = {}) {
	const timers = [];
	const events = [];
	const link = {
		click() {
			events.push('click');
			if (options.clickFails) throw new Error('click refused');
		},
		remove() {
			events.push('remove');
			if (options.removeFails) throw new Error('remove refused');
		},
	};
	const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
	Object.defineProperty(globalThis, 'document', {
		configurable: true,
		value: {
			createElement(tag) {
				assert.equal(tag, 'a');
				return link;
			},
			body: { append: (value) => assert.equal(value, link) },
		},
	});
	t.after(() => {
		if (previous) Object.defineProperty(globalThis, 'document', previous);
		else delete globalThis.document;
	});
	t.mock.method(URL, 'createObjectURL', (blob) => {
		assert.equal(blob.type, 'image/vnd-ms.dds');
		events.push('create');
		if (options.createFails) throw new Error('Blob URL refused');
		return 'blob:owned-dds';
	});
	t.mock.method(URL, 'revokeObjectURL', (url) => {
		assert.equal(url, 'blob:owned-dds');
		events.push('revoke');
		if (options.revokeFails) throw new Error('retirement unknown');
	});
	t.mock.method(globalThis, 'setTimeout', (callback, delay) => {
		timers.push({ callback, delay });
	});
	return { timers, events, link };
}
const download = () =>
	downloadCopyTracked(
		new Uint8Array([1, 2, 3]),
		'copy.dds',
		'image/vnd-ms.dds',
	);

test('tracked download stays pending until URL retirement, not click completion', async (t) => {
	const { timers, events, link } = environment(t);
	let settled = false;
	const result = download().then((value) => {
		settled = true;
		return value;
	});
	await Promise.resolve();
	assert.equal(settled, false);
	assert.equal(link.download, 'copy.dds');
	assert.equal(link.hidden, true);
	assert.deepEqual(events, ['create', 'click', 'remove']);
	assert.equal(timers.length, 1);
	assert.equal(timers[0].delay, 1000);
	timers[0].callback();
	assert.deepEqual(await result, { requested: true });
	assert.deepEqual(events, ['create', 'click', 'remove', 'revoke']);
});

test('refused click still retires its URL before reporting a known refusal', async (t) => {
	const { timers, events } = environment(t, { clickFails: true });
	const result = download();
	assert.equal(timers.length, 1);
	assert.equal(timers[0].delay, 0);
	timers[0].callback();
	assert.deepEqual(await result, { requested: false });
	assert.deepEqual(events, ['create', 'click', 'remove', 'revoke']);
});

test('failure before URL acquisition is a completed refusal with no timer', async (t) => {
	const { timers, events } = environment(t, { createFails: true });
	assert.deepEqual(await download(), { requested: false });
	assert.deepEqual(events, ['create']);
	assert.equal(timers.length, 0);
});

test('DOM cleanup failure cannot skip URL retirement', async (t) => {
	const { timers, events } = environment(t, { removeFails: true });
	const result = download();
	timers[0].callback();
	assert.deepEqual(await result, { requested: true });
	assert.equal(events.at(-1), 'revoke');
});

test('unknown URL retirement rejects instead of releasing a known-complete lease', async (t) => {
	const { timers } = environment(t, { revokeFails: true });
	const result = download();
	const rejection = assert.rejects(result, /retirement unknown/);
	timers[0].callback();
	await rejection;
});
