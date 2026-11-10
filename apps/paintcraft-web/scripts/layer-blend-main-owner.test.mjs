import test from 'node:test';
import assert from 'node:assert/strict';
import {
	mainBlendOwner8Facade,
	mainBlendRetirementTick8,
	deferMainBlendCredit8,
} from '../src/lib/layer-blend-main-owner.js';

test('exact frozen facade delegates receiver/request and passes actual guard through', () => {
	const request = Object.freeze({ token: '1' }),
		guard = { release() {} };
	const owner = {
		admitFrame(n, v) {
			assert.equal(this, owner);
			assert.equal(n, 16);
			assert.equal(v, request);
			return guard;
		},
		current(v) {
			assert.equal(this, owner);
			assert.equal(v, request);
			return true;
		},
		retire() {
			assert.equal(this, owner);
			return 7;
		},
	};
	const facade = mainBlendOwner8Facade(owner);
	assert.deepEqual(Object.keys(facade), ['admitFrame', 'current', 'retire']);
	assert(Object.isFrozen(facade));
	assert.equal(facade.admitFrame(16, request), guard);
	assert.equal(facade.current(request), true);
	assert.equal(facade.retire(), 7);
});
test('retire is consuming even while its original promise is unresolved', async () => {
	let finish;
	const retired = new Promise((r) => (finish = r));
	const owner = {
		admitFrame() {},
		current() {
			return true;
		},
		retire() {
			return retired;
		},
	};
	const facade = mainBlendOwner8Facade(owner);
	assert.equal(facade.retire(), retired);
	for (const call of [
		() => facade.admitFrame(0, {}),
		() => facade.current({}),
		() => facade.retire(),
	])
		assert.throws(call, /consumed/);
	finish();
	await retired;
});
test('actual thrown values and returned rejection identity pass unchanged', async () => {
	const original = { code: 'resource.limit' },
		promise = Promise.reject(original);
	promise.catch(() => {});
	const owner = {
		admitFrame() {
			throw original;
		},
		current() {
			return false;
		},
		retire() {
			return promise;
		},
	};
	const facade = mainBlendOwner8Facade(owner);
	assert.throws(
		() => facade.admitFrame(1, {}),
		(e) => e === original,
	);
	assert.equal(facade.current({}), false);
	assert.equal(facade.retire(), promise);
	await assert.rejects(promise, (e) => e === original);
});
test('credit is a native microtask with captured method and actual receiver', async () => {
	let calls = 0;
	const handle = {
		release() {
			assert.equal(this, handle);
			calls++;
			return 19;
		},
	};
	const pending = deferMainBlendCredit8(handle);
	assert.equal(calls, 0);
	handle.release = () =>
		assert.fail('method must be captured before scheduling');
	assert.equal(await pending, 19);
	assert.equal(calls, 1);
});
test('original credit release failure rejects without fallback or a second call', async () => {
	const original = new Error('original release');
	let calls = 0;
	const pending = deferMainBlendCredit8({
		release() {
			calls++;
			throw original;
		},
	});
	assert.equal(calls, 0);
	await assert.rejects(pending, (e) => e === original);
	assert.equal(calls, 1);
});
test('scheduling exception never releases synchronously', {
	concurrency: false,
}, (t) => {
	const original = new Error('schedule failure');
	let calls = 0;
	const mocked = t.mock.method(Promise, 'resolve', () => {
		throw original;
	});
	try {
		assert.throws(
			() =>
				deferMainBlendCredit8({
					release() {
						calls++;
					},
				}),
			(e) => e === original,
		);
		assert.equal(calls, 0);
	} finally {
		mocked.mock.restore();
	}
});
test('tick schedules exactly one configured25ms callback and resolves asynchronously', {
	concurrency: false,
}, async (t) => {
	let callback,
		count = 0;
	const mocked = t.mock.method(globalThis, 'setTimeout', (fn, delay) => {
		count++;
		assert.equal(delay, 25);
		callback = fn;
		return 1;
	});
	try {
		let done = false;
		const pending = mainBlendRetirementTick8().then(() => (done = true));
		assert.equal(count, 1);
		assert.equal(done, false);
		callback();
		assert.equal(done, false);
		await pending;
		assert.equal(done, true);
	} finally {
		mocked.mock.restore();
	}
});
test('invalid handles refuse without creating owner or fallback', () => {
	assert.throws(() => mainBlendOwner8Facade(null), TypeError);
	assert.throws(
		() => mainBlendOwner8Facade({ admitFrame() {}, current() {} }),
		TypeError,
	);
	assert.throws(() => deferMainBlendCredit8({}), TypeError);
});
