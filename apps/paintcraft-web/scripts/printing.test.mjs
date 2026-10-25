import test from 'node:test';
import assert from 'node:assert/strict';
import { BrowserPrinter } from '../src/lib/printing.js';

function deferred() {
	let resolve;
	let reject;
	const promise = new Promise((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

function browser({ decoded = Promise.resolve(), mode = 'async' } = {}) {
	const created = [];
	const revoked = [];
	const frames = [];
	const focus = {
		calls: 0,
		focus() {
			this.calls++;
		},
	};
	const nodes = () => ({
		children: [],
		append(node) {
			this.children.push(node);
		},
	});
	const doc = {
		activeElement: focus,
		body: {
			append(frame) {
				frames.push(frame);
				queueMicrotask(() => frame.onload?.());
			},
		},
		createElement(tag) {
			assert.equal(tag, 'iframe');
			const win = new EventTarget();
			win.calls = 0;
			win.focus = () => {};
			win.print = () => {
				win.calls++;
				if (mode === 'throw') throw new Error('Denied');
				if (mode === 'ignore') return;
				win.dispatchEvent(new Event('beforeprint'));
				if (mode === 'sync') win.dispatchEvent(new Event('afterprint'));
			};
			return {
				style: {},
				removed: 0,
				contentWindow: win,
				contentDocument: {
					head: nodes(),
					body: nodes(),
					createElement(kind) {
						if (kind === 'img') return { decode: () => decoded };
						assert.equal(kind, 'style');
						return {};
					},
				},
				setAttribute() {},
				remove() {
					this.removed++;
				},
			};
		},
	};
	const printer = new BrowserPrinter(doc, {
		createObjectURL(blob) {
			created.push(blob);
			return `blob:${created.length}`;
		},
		revokeObjectURL(url) {
			revoked.push(url);
		},
	});
	return { printer, frames, created, revoked, focus };
}

const drain = async () => {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
};

test('capture waits for decoding, owns exact PNG bytes and cleans up only after print', async () => {
	const decoded = deferred();
	const b = browser({ decoded: decoded.promise });
	const bytes = new Uint8Array([1, 2, 3, 4]);
	const title = '<script>Not markup</script> 日本語';
	const done = b.printer.print(bytes, title);
	await drain();
	const frame = b.frames[0];
	assert.equal(frame.contentWindow.calls, 0);
	assert.equal(frame.contentDocument.title, title);
	assert.equal(frame.contentDocument.body.children.length, 1);
	assert.equal(frame.contentDocument.body.children[0].alt, title);
	assert.equal(b.created[0].type, 'image/png');
	assert.deepEqual(new Uint8Array(await b.created[0].arrayBuffer()), bytes);
	decoded.resolve();
	await drain();
	assert.equal(frame.contentWindow.calls, 1);
	assert.ok(
		b.focus.calls > 0,
		'editor focus returns after a nonblocking print request',
	);
	assert.deepEqual(b.revoked, []);
	await assert.rejects(b.printer.print(bytes, 'Duplicate'), /busy/);
	frame.contentWindow.dispatchEvent(new Event('afterprint'));
	assert.equal(await done, 'closed');
	assert.equal(frame.removed, 1);
	assert.deepEqual(b.revoked, ['blob:1']);
	b.printer.cancel();
	assert.equal(frame.removed, 1);
});

test('cancel during decode fences late completion away from the next owned job', async () => {
	const decoded = deferred();
	const b = browser({ decoded: decoded.promise });
	const first = b.printer.print(new Uint8Array([1]), 'First');
	await drain();
	b.printer.cancel();
	assert.equal(await first, 'cancelled');
	const second = b.printer.print(new Uint8Array([2]), 'Second');
	await drain();
	decoded.resolve();
	await drain();
	assert.equal(b.frames[0].contentWindow.calls, 0);
	assert.equal(b.frames[1].contentWindow.calls, 1);
	b.frames[0].contentWindow.dispatchEvent(new Event('afterprint'));
	assert.notEqual(b.printer.active, null);
	b.frames[1].contentWindow.dispatchEvent(new Event('afterprint'));
	assert.equal(await second, 'closed');
	assert.deepEqual(b.revoked, ['blob:1', 'blob:2']);
});

test('synchronous dialog completion and cancellation release every owned resource', async () => {
	const b = browser({ mode: 'sync' });
	assert.equal(await b.printer.print(new Uint8Array([1]), 'Sync'), 'closed');
	assert.equal(b.frames[0].removed, 1);
	assert.deepEqual(b.revoked, ['blob:1']);
	const async = browser();
	const done = async.printer.print(new Uint8Array([1]), 'Cancel');
	await drain();
	async.printer.cancel();
	assert.equal(await done, 'cancelled');
	assert.deepEqual(async.revoked, ['blob:1']);
});

test('silent refusal, thrown print failures and decode rejection retain no frame or URL', async () => {
	for (const mode of ['ignore', 'throw']) {
		const b = browser({ mode });
		await assert.rejects(b.printer.print(new Uint8Array([1]), 'Denied'));
		assert.equal(b.frames[0].removed, 1);
		assert.deepEqual(b.revoked, ['blob:1']);
		assert.equal(b.printer.active, null);
	}
	const decoded = deferred();
	const b = browser({ decoded: decoded.promise });
	const done = b.printer.print(new Uint8Array([1]), 'Invalid PNG');
	await drain();
	decoded.reject(new Error('Invalid image'));
	await assert.rejects(done, /Invalid image/);
	assert.equal(b.frames[0].contentWindow.calls, 0);
	assert.equal(b.frames[0].removed, 1);
	assert.deepEqual(b.revoked, ['blob:1']);
});

test('preparation timeout releases resources, but an open dialog has no arbitrary expiry', async (t) => {
	t.mock.timers.enable({ apis: ['setTimeout'] });
	const b = browser({ decoded: new Promise(() => {}) });
	const done = b.printer.print(new Uint8Array([1]), 'Never decoded');
	await drain();
	t.mock.timers.tick(30000);
	await assert.rejects(done, /did not become ready/);
	assert.equal(b.frames[0].removed, 1);
	assert.deepEqual(b.revoked, ['blob:1']);
	const ready = browser();
	const printing = ready.printer.print(new Uint8Array([1]), 'Open dialog');
	await drain();
	t.mock.timers.tick(600000);
	assert.notEqual(ready.printer.active, null);
	ready.printer.cancel();
	assert.equal(await printing, 'cancelled');
});

test('missing browser document fails without acquiring resources', async () => {
	await assert.rejects(
		new BrowserPrinter({}).print(new Uint8Array(), 'No document'),
	);
});
