import test from 'node:test';
import assert from 'node:assert/strict';
import {
	fixture,
	manifest,
	PROFILE,
	PIXELS,
} from './layer-blend-runtime-fixture.mjs';
import { createRenderAdmission } from '../src/lib/render-admission.js';
import { createLayerBlendController8 } from '../src/lib/layer-blend-preview-client.js';
import {
	compiledFacts8,
	profile8,
	window8,
	getterU648,
	CONTROL_JS8,
} from '../src/lib/layer-blend-preview-wire.js';
const input = () => Uint8Array.from([1, 2, 3, 4]).buffer;
function setup(failure = '') {
	const f = fixture(failure),
		broker = createRenderAdmission();
	const controller = createLayerBlendController8(
		broker,
		() => new f.Worker(),
		2000,
	).begin();
	return { ...f, broker, controller };
}
function mainOwner(f, credit, current = () => true) {
	return {
		admitFrame(bytes) {
			f.events.push(`Main:admit:${bytes}`);
			return {
				release() {
					f.events.push('Main:frame-release');
				},
			};
		},
		current,
		retire() {
			f.events.push('Main:retire');
			credit.release();
		},
	};
}

test('actual primitive getter adaptation and fixed profile framing match independent digest', async () => {
	const f = fixture(),
		ready = compiledFacts8(f.api);
	assert.equal(await profile8(ready.runtimeFacts), PROFILE);
	assert.equal(ready.runtimeFacts.limits.noiseWork, '34359738368');
	assert.equal(ready.runtimeFacts.limits.outputCells, 4194304);
	assert.equal(CONTROL_JS8, 57856);
	assert.equal(getterU648(-1n), 18446744073709551615n);
	assert.equal(getterU648(-9223372036854775808n), 9223372036854775808n);
	assert.throws(() => getterU648(-9223372036854775809n));
});

test('512-byte window exact UUID/role/u64 offsets and zeroed full/header-only close', () => {
	const m = manifest(),
		bytes = window8('open', 9, m),
		d = new DataView(bytes.buffer);
	assert.equal(bytes.length, 512);
	assert.deepEqual([...bytes.subarray(0, 8)], [80, 67, 56, 87, 1, 0, 1, 4]);
	assert.equal(d.getUint32(8, true), 9);
	assert.equal(d.getBigUint64(48, true), 19n);
	assert.deepEqual([...bytes.subarray(16, 32)], Array(16).fill(17));
	assert.equal(d.getBigUint64(176, true), 4n);
	assert.equal(d.getBigUint64(208, true), 4096n);
	assert.equal(d.getBigUint64(296, true), 7168n);
	assert(bytes.subarray(304).every((n) => n === 0));
	const view = {
		token: '3',
		generation: '5',
		visible: [0, 0, 2, 1],
		level: 0,
		ordinal: 7,
	};
	const ack = window8('ack', 10, m, view),
		a = new DataView(ack.buffer);
	assert.equal(a.getBigUint64(304, true), 3n);
	assert.equal(a.getUint32(340, true), 7);
	assert(
		window8('close', 11, m, view)
			.subarray(304)
			.every((n) => n === 0),
	);
	assert(
		window8('close', 11, null)
			.subarray(16)
			.every((n) => n === 0),
	);
});

test('real JS modules transfer staged bytes, copy exact pixels before ACK, retire Main before broker release', async () => {
	const f = setup();
	await f.controller.ready;
	assert.equal(f.broker.usage().reserved, 469762048 + 7168 + 57856);
	const credit = f.controller.reserveMain(67108864),
		source = input();
	const session = await f.controller.open(
		manifest(),
		source,
		mainOwner(f, credit),
	);
	assert.equal(
		source.byteLength,
		0,
		'actual structuredClone transfer detaches',
	);
	const frame = await session.view({ visible: [0, 0, 2, 1], level: 0 });
	assert.deepEqual([...new Uint8Array(frame.rgba)], PIXELS);
	assert(f.events.indexOf('stage') < f.events.indexOf('sourcePtr'));
	assert(f.events.indexOf('sourceLen') < f.events.indexOf('seal'));
	assert(f.events.indexOf('Main:admit:8') < f.events.indexOf('beginView'));
	assert(f.events.indexOf('message:patch') < f.events.indexOf('ack'));
	assert.equal(f.events.filter((x) => x === 'beginView').length, 1);
	await assert.rejects(
		session.view({ visible: [0, 0, 2, 1], level: 0 }),
		/bounded Main view/,
	);
	frame.release();
	await f.controller.close();
	assert.equal(f.live(), 0);
	assert.deepEqual(f.broker.usage(), { reserved: 0, quarantined: false });
	assert(f.events.indexOf('close') < f.events.indexOf('Main:retire'));
});

test('borrowed source view is formed after getters that can grow actual nonshared memory', async () => {
	const f = setup('grow-before-fill');
	await f.controller.ready;
	const credit = f.controller.reserveMain(67108864);
	await f.controller.open(manifest(), input(), mainOwner(f, credit));
	assert.equal(f.memory.buffer.byteLength, 33 * 65536);
	await f.controller.close();
	assert.equal(f.broker.usage().reserved, 0);
});

test('typed constructor/stage/seal failure joins genuine cleanup without fake OPENED', async () => {
	for (const phase of ['constructor', 'stage', 'seal']) {
		const f = setup(phase);
		await f.controller.ready;
		const credit = f.controller.reserveMain(67108864);
		await assert.rejects(
			f.controller.open(manifest(), input(), mainOwner(f, credit)),
			(e) =>
				e.code === 'codec.failed' &&
				e.operationIndex === 7 &&
				e.closed === true,
		);
		assert.equal(f.events.includes('message:opened'), false);
		if (phase !== 'constructor')
			assert(
				f.events.indexOf('close') < f.events.indexOf('message:error'),
			);
		await f.controller.close();
		assert.equal(f.live(), 0);
		assert.equal(f.broker.usage().reserved, 0);
	}
});

test('constructor trap and consuming-close error quarantine rather than refund from live count', async () => {
	const trap = setup('constructor-trap');
	await trap.controller.ready;
	const c = trap.controller.reserveMain(67108864);
	await assert.rejects(
		trap.controller.open(manifest(), input(), mainOwner(trap, c)),
	);
	await assert.rejects(trap.controller.close());
	assert.equal(trap.broker.usage().quarantined, true);
	const close = setup('close');
	await close.controller.ready;
	const credit = close.controller.reserveMain(67108864);
	await close.controller.open(manifest(), input(), mainOwner(close, credit));
	await assert.rejects(close.controller.close());
	assert.equal(close.live(), 0);
	assert.equal(close.broker.usage().quarantined, true);
});

test('abandonment before OPEN closes empty only, without constructing any Rust source instance', async () => {
	const f = setup();
	await f.controller.ready;
	await f.controller.close();
	assert.equal(f.events.includes('constructor'), false);
	assert.equal(f.events.includes('message:closed-empty'), true);
	assert.equal(f.broker.usage().reserved, 0);
});

test('close during Main admission joins pending owner and never dispatches an unadmitted VIEW', async () => {
	const f = setup();
	await f.controller.ready;
	const credit = f.controller.reserveMain(67108864);
	let releaseAdmission;
	const owner = mainOwner(f, credit);
	owner.admitFrame = () =>
		new Promise((resolve) => {
			releaseAdmission = resolve;
		});
	const session = await f.controller.open(manifest(), input(), owner);
	const view = session.view({ visible: [0, 0, 2, 1], level: 0 });
	const closed = f.controller.close();
	releaseAdmission({
		release() {
			f.events.push('Main:delayed-release');
		},
	});
	await assert.rejects(view);
	await closed;
	assert.equal(f.events.includes('beginView'), false);
	assert(
		f.events.indexOf('Main:delayed-release') <
			f.events.indexOf('Main:retire'),
	);
	assert.equal(f.broker.usage().reserved, 0);
});

test('stale public owner cancels the actual operation without copy ACK or publication', async () => {
	const f = setup();
	await f.controller.ready;
	const credit = f.controller.reserveMain(67108864);
	const session = await f.controller.open(
		manifest(),
		input(),
		mainOwner(f, credit, () => !f.events.includes('message:patch')),
	);
	await assert.rejects(
		session.view({ visible: [0, 0, 2, 1], level: 0 }),
		/cancelled/,
	);
	assert.equal(f.events.includes('ack'), false);
	assert.equal(f.events.includes('cancel'), true);
	assert.equal(f.events.includes('Main:frame-release'), true);
	await f.controller.close();
	assert.equal(f.broker.usage().reserved, 0);
});

test('Main admission failure never dispatches VIEW or allocates an implicit replacement grant', async () => {
	const f = setup();
	await f.controller.ready;
	const credit = f.controller.reserveMain(67108864),
		owner = mainOwner(f, credit);
	owner.admitFrame = () => {
		throw Object.assign(new Error('original Main scope exhausted'), {
			code: 'ResourceLimit',
		});
	};
	const session = await f.controller.open(manifest(), input(), owner);
	await assert.rejects(
		session.view({ visible: [0, 0, 2, 1], level: 0 }),
		/original Main scope/,
	);
	assert.equal(f.events.includes('beginView'), false);
	await f.controller.close();
	assert.equal(f.events.filter((e) => e === 'Main:retire').length, 1);
	assert.equal(f.broker.usage().reserved, 0);
});

test('missing READY times out into quarantine with no replacement broker or release', async () => {
	const broker = createRenderAdmission();
	let terminated = false;
	const controller = createLayerBlendController8(
		broker,
		() => ({
			postMessage() {},
			terminate() {
				terminated = true;
			},
		}),
		20,
	).begin();
	await assert.rejects(controller.ready, /missing joined response/);
	await assert.rejects(controller.close());
	assert.equal(terminated, true);
	assert.equal(broker.usage().quarantined, true);
	assert.equal(broker.usage().reserved, 469762048 + 57856);
	assert.throws(() => broker.acquire(1), /busy or quarantined/);
});

test('earlier COMPLETE retains pending CANCEL join until actual unjoined response', async () => {
	const { blendChannel8 } = await import(
		'../src/lib/layer-blend-preview-channel.js'
	);
	const { binding8, sourceIdentity8 } = await import(
		'../src/lib/layer-blend-preview-failure.js'
	);
	const sent = [];
	let quarantined = false;
	const worker = { postMessage: (m) => sent.push(m.control), terminate() {} };
	const channel = blendChannel8(
		worker,
		{
			quarantine() {
				quarantined = true;
			},
			release() {
				assert.fail('unjoined must not release');
			},
		},
		2000,
	);
	worker.onmessage({ data: compiledFacts8(fixture().api) });
	await channel.ready;
	const open = manifest();
	const opening = channel.open(open, input());
	worker.onmessage({
		data: {
			kind: 'opened',
			protocol: 8,
			id: sent.at(-1).id,
			...sourceIdentity8(open),
			byteLength: open.byteLength,
			canvas: open.canvas,
			storedNonNormal: true,
			storedNoise: false,
		},
	});
	await opening;
	const view = {
		kind: 'view',
		protocol: 8,
		id: 1,
		...binding8(open),
		token: '1',
		generation: '1',
		visible: [0, 0, 2, 1],
		level: 0,
	};
	const rendering = channel.view(view, () => {});
	const viewId = sent.at(-1).id;
	const cancelling = channel.cancel({
		kind: 'cancel',
		protocol: 8,
		id: 1,
		...binding8(open, view),
	});
	const rejected = assert.rejects(cancelling, /unjoined/);
	worker.onmessage({
		data: {
			kind: 'complete',
			protocol: 8,
			id: viewId,
			...binding8(open, view),
			patchCount: 1,
		},
	});
	await rendering;
	worker.onmessage({ data: { kind: 'unjoined', protocol: 8 } });
	await rejected;
	assert.equal(quarantined, true);
});

test('immediate controller close joins READY and compiled-H before empty close/refund', async () => {
	const f = setup();
	// Deliberately do not await controller.ready before requesting retirement.
	const closing = f.controller.close();
	assert.equal(f.broker.usage().quarantined, false);
	await closing;
	await f.controller.ready;
	assert.equal(f.events.includes('constructor'), false);
	assert(
		f.events.indexOf('message:ready') <
			f.events.indexOf('message:closed-empty'),
	);
	assert(
		f.events.indexOf('message:closed-empty') <
			f.events.indexOf('terminate'),
	);
	assert.deepEqual(f.broker.usage(), { reserved: 0, quarantined: false });
});
