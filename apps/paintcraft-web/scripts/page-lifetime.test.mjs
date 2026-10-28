import assert from 'node:assert/strict';
import test from 'node:test';
import {
	reportStartupFailure,
	startOwnedPage,
} from '../src/lib/page-lifetime.js';

function fixture() {
	const abort = new AbortController();
	const target = new EventTarget();
	const calls = [];
	const status = { hidden: false, textContent: 'loading' };
	let resolve;
	let reject;
	let freed = false;
	const pending = new Promise((yes, no) => {
		resolve = yes;
		reject = no;
	});
	const live = (name) => {
		assert.equal(freed, false, `${name} called after free`);
		calls.push(name);
	};
	const app = {
		start(canvas) {
			live('start');
			assert.equal(canvas, 'canvas');
			return pending;
		},
		application_version() {
			live('version');
			return '0.1.0';
		},
		install_codec_service(service) {
			live('install');
			assert.equal(service, 'service');
		},
		is_dirty() {
			live('dirty');
			return true;
		},
		destroy() {
			live('destroy');
		},
	};
	const run = () =>
		startOwnedPage({
			app,
			canvas: 'canvas',
			lifetime: abort.signal,
			createService(version) {
				assert.equal(version, '0.1.0');
				calls.push('factory');
				return 'service';
			},
			status,
			target,
		});
	return {
		abort,
		target,
		calls,
		status,
		app,
		run,
		resolve,
		reject,
		free() {
			freed = true;
			abort.abort();
		},
	};
}
function pagehide(persisted) {
	const event = new Event('pagehide');
	Object.defineProperty(event, 'persisted', { value: persisted });
	return event;
}

test('live startup installs service and guards, preserving BFCache and normal dirty warning', async () => {
	const f = fixture();
	const ready = f.run();
	f.resolve();
	assert.equal(await ready, 'ready');
	assert.equal(f.status.hidden, true);
	assert.deepEqual(f.calls, ['start', 'version', 'factory', 'install']);
	const unload = new Event('beforeunload', { cancelable: true });
	f.target.dispatchEvent(unload);
	assert.equal(unload.defaultPrevented, true);
	f.target.dispatchEvent(pagehide(true));
	assert.equal(f.calls.filter((name) => name === 'destroy').length, 0);
	f.target.dispatchEvent(pagehide(false));
	assert.equal(f.calls.filter((name) => name === 'destroy').length, 1);
});

test('public owner free removes actual EventTarget host guards before later unload', async () => {
	const f = fixture();
	const ready = f.run();
	f.resolve();
	await ready;
	f.free();
	const before = [...f.calls];
	const unload = new Event('beforeunload', { cancelable: true });
	f.target.dispatchEvent(unload);
	f.target.dispatchEvent(pagehide(false));
	assert.equal(unload.defaultPrevented, false);
	assert.deepEqual(f.calls, before);
});

for (const settles of ['resolve', 'reject']) {
	test(`free while startup awaits ${settles} exits quietly without calling freed handle`, async () => {
		const f = fixture();
		const ready = f.run();
		f.free();
		if (settles === 'resolve') f.resolve();
		else f.reject(new Error('cancelled async graphics'));
		assert.equal(await ready, 'disposed');
		const logger = {
			error() {
				assert.fail('disposed owner reported startup failure');
			},
		};
		assert.equal(
			reportStartupFailure(
				f.app,
				f.abort.signal,
				f.status,
				new Error('late'),
				logger,
			),
			'disposed',
		);
		assert.deepEqual(f.calls, ['start']);
		assert.equal(f.status.hidden, false);
		assert.equal(f.status.textContent, 'loading');
		f.target.dispatchEvent(pagehide(false));
	});
}

test('live startup rejection retains original error and cleans live owner once', async () => {
	const f = fixture();
	const original = new Error('graphics initialization failed');
	const ready = f.run();
	f.reject(original);
	await assert.rejects(ready, (error) => error === original);
	const reported = [];
	assert.equal(
		reportStartupFailure(f.app, f.abort.signal, f.status, original, {
			error(error) {
				reported.push(error);
			},
		}),
		'failed',
	);
	assert.deepEqual(reported, [original]);
	assert.deepEqual(f.calls, ['start', 'destroy']);
	assert.ok(f.status.textContent.endsWith(String(original)));
});

test('failure before public owner construction reports original error without a fake cleanup', () => {
	const status = {};
	const original = new Error('font load failed');
	const reported = [];
	assert.equal(
		reportStartupFailure(undefined, undefined, status, original, {
			error(error) {
				reported.push(error);
			},
		}),
		'failed',
	);
	assert.deepEqual(reported, [original]);
	assert.ok(status.textContent.endsWith(String(original)));
});
