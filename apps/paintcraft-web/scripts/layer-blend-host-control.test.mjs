// Real broker/client modules; Worker ABI fixture is an explicit contract double.
// This does not qualify compiled Rust C, future allocation, or Main publication.
import assert from 'node:assert/strict';
import test from 'node:test';
import { createRenderAdmission } from '../src/lib/render-admission.js';
import { createLayerBlendController8 } from '../src/lib/layer-blend-preview-client.js';
import { CONTROL_JS8 } from '../src/lib/layer-blend-preview-wire.js';
import { deferMainBlendCredit8 } from '../src/lib/layer-blend-main-owner.js';
import { fixture, manifest } from './layer-blend-runtime-fixture.mjs';
const BASE = 448 * 1024 * 1024;
// Deliberately unrelated to any Rust fixture/compiled quote: transport-only bytes.
const DECLARED = 23457;
const resource = (f) => assert.throws(f, { code: 'ResourceLimit' });
function setup(failure = '') {
	const f = fixture(failure),
		broker = createRenderAdmission();
	const pair = createLayerBlendController8(
		broker,
		() => {
			assert.equal(
				broker.usage().reserved,
				BASE + CONTROL_JS8 + DECLARED,
			);
			return new f.Worker();
		},
		2000,
	).beginWithHostControl(DECLARED);
	return { ...f, broker, ...pair };
}
function owner(credit) {
	return {
		admitFrame() {
			return { release() {} };
		},
		current() {
			return true;
		},
		retire() {
			credit.release();
		},
	};
}
test('host guard is exact own data, one-shot before Main, and either retirement order is safe', () => {
	for (const hostFirst of [false, true]) {
		const broker = createRenderAdmission(),
			lease = broker.acquire(BASE + DECLARED);
		const host = lease.reserveHostControl(DECLARED);
		assert.deepEqual(Object.keys(host), ['byteLength', 'release']);
		assert.equal(
			Object.getOwnPropertyDescriptor(host, 'byteLength').value,
			DECLARED,
		);
		resource(() => lease.reserveHostControl(1));
		const main = lease.reserveMain(1);
		if (hostFirst) host.release();
		lease.release();
		assert.equal(broker.usage().reserved, BASE + DECLARED);
		main.release();
		if (!hostFirst) {
			assert.equal(broker.usage().reserved, BASE + DECLARED);
			host.release();
		}
		host.release();
		assert.equal(broker.usage().reserved, 0);
		const next = broker.acquire(1);
		host.release();
		assert.equal(broker.usage().reserved, 1);
		next.release();
	}
});
test('invalid declarations refuse before factory; late and repeated host grants refuse', () => {
	const broker = createRenderAdmission();
	let created = 0;
	const api = createLayerBlendController8(broker, () => {
		created++;
		throw new Error('unexpected');
	});
	for (const n of [
		0,
		-1,
		0.5,
		NaN,
		Infinity,
		'23457',
		Number.MAX_SAFE_INTEGER,
	])
		assert.throws(() => api.beginWithHostControl(n), {
			code: 'InputSchema',
		});
	assert.equal(created, 0);
	assert.equal(broker.usage().reserved, 0);
	const lease = broker.acquire(100);
	for (const n of [0, -1, 101, NaN, '1'])
		resource(() => lease.reserveHostControl(n));
	const host = lease.reserveHostControl(1);
	host.release();
	resource(() => lease.reserveHostControl(1));
	lease.release();
	const later = broker.acquire(100),
		main = later.reserveMain(1);
	resource(() => later.reserveHostControl(1));
	main.release();
	later.release();
});
test('genuine READY growth retains initial C and close resolves before its host guard drops', async () => {
	const f = setup();
	assert.deepEqual(
		Object.keys({ controller: f.controller, hostControl: f.hostControl }),
		['controller', 'hostControl'],
	);
	await f.controller.ready;
	assert.equal(
		f.broker.usage().reserved,
		BASE + CONTROL_JS8 + DECLARED + 7168,
	);
	const main = f.controller.reserveMain(64 * 1024 * 1024);
	await f.controller.open(
		manifest(),
		Uint8Array.from([1, 2, 3, 4]).buffer,
		owner(main),
	);
	await f.controller.close();
	assert.equal(f.live(), 0);
	assert.equal(
		f.broker.usage().reserved,
		BASE + CONTROL_JS8 + DECLARED + 7168,
	);
	resource(() => f.broker.acquire(1));
	// Models the actual future's payload retirement before its deferred native release.
	let payload = true;
	payload = false;
	const released = deferMainBlendCredit8({
		release() {
			assert.equal(payload, false);
			f.hostControl.release();
		},
	});
	assert.notEqual(f.broker.usage().reserved, 0);
	await released;
	assert.equal(f.broker.usage().reserved, 0);
});
test('pre-Worker factory throw releases host and serialization credit without a fake join', () => {
	const broker = createRenderAdmission();
	const failure = new Error('factory before Worker');
	assert.throws(
		() =>
			createLayerBlendController8(broker, () => {
				assert.equal(
					broker.usage().reserved,
					BASE + CONTROL_JS8 + DECLARED,
				);
				throw failure;
			}).beginWithHostControl(DECLARED),
		(e) => e === failure,
	);
	assert.equal(broker.usage().reserved, 0);
});
test('a returned Worker setup failure remains quarantined', () => {
	const broker = createRenderAdmission();
	let terminated = 0;
	const worker = {
		terminate() {
			terminated++;
		},
		set onerror(_) {
			throw new Error('setup');
		},
	};
	assert.throws(
		() =>
			createLayerBlendController8(
				broker,
				() => worker,
				10,
			).beginWithHostControl(DECLARED),
		/setup/,
	);
	assert.equal(terminated, 1);
	assert.deepEqual(broker.usage(), {
		reserved: BASE + CONTROL_JS8 + DECLARED,
		quarantined: true,
	});
});
test('malformed READY rejects close and releasing host never returns quarantined credit', async () => {
	const broker = createRenderAdmission();
	let worker;
	const pair = createLayerBlendController8(
		broker,
		() => {
			worker = { terminate() {}, postMessage() {} };
			return worker;
		},
		2000,
	).beginWithHostControl(DECLARED);
	const failure = assert.rejects(pair.controller.ready);
	worker.onmessage({ data: { kind: 'ready' } });
	await failure;
	await assert.rejects(pair.controller.close());
	pair.hostControl.release();
	pair.hostControl.release();
	assert.equal(broker.usage().quarantined, true);
	resource(() => broker.acquire(1));
});
test('unknown Rust trap quarantines after Main retire and host release', async () => {
	const f = setup('constructor-trap');
	await f.controller.ready;
	const main = f.controller.reserveMain(64 * 1024 * 1024);
	await assert.rejects(
		f.controller.open(
			manifest(),
			Uint8Array.from([1, 2, 3, 4]).buffer,
			owner(main),
		),
	);
	await assert.rejects(f.controller.close());
	f.hostControl.release();
	assert.equal(f.broker.usage().quarantined, true);
	resource(() => f.broker.acquire(1));
});
test('old zero-host begin has original return shape and joined retirement', async () => {
	const f = fixture(),
		broker = createRenderAdmission();
	const controller = createLayerBlendController8(
		broker,
		() => new f.Worker(),
		2000,
	).begin();
	assert.equal(Object.hasOwn(controller, 'hostControl'), false);
	assert.equal(Object.hasOwn(controller, 'controller'), false);
	await controller.ready;
	await controller.close();
	assert.equal(broker.usage().reserved, 0);
});
