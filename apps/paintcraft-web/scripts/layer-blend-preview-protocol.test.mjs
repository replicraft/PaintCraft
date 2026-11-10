import assert from 'node:assert/strict';
import test from 'node:test';
import { decimal8 } from '../src/lib/layer-blend-preview-common.js';
import {
	geometry8,
	LIMITS8,
	patchRect8,
	VERSIONS8,
	validateAck8,
	validateClosed8,
	validateComplete8,
	validateOpen8,
	validateOpened8,
	validatePatch8,
	validateReady8,
	validateSourceBuffer8,
	validateView8,
} from '../src/lib/layer-blend-preview-protocol.js';
import { Protocol8Sequence } from '../src/lib/layer-blend-preview-sequence.js';

const uuid = (n) =>
	`${n.repeat(8)}-${n.repeat(4)}-${n.repeat(4)}-${n.repeat(4)}-${n.repeat(12)}`;
const identity = () => ({
	owner: uuid('1'),
	publicTicket: { state: uuid('2'), epoch: '19' },
	document: uuid('3'),
	sourceState: uuid('4'),
	draft: uuid('5'),
	sourceSha256: 'a'.repeat(64),
	profileSha256: 'b'.repeat(64),
});
function open() {
	return {
		kind: 'open',
		protocol: 8,
		id: 1,
		...identity(),
		byteLength: '4096',
		canvas: [130, 66],
		storedNonNormal: true,
		storedNoise: false,
		mode: 'base',
		roles: {
			E: '8192',
			P: '2097152',
			M: '1048576',
			A: '1048576',
			I: '0',
			C: '1048576',
			Q: '262144',
			K: '4096',
			W: '4194304',
			F: '16384',
			O: '2097152',
			H: '16384',
		},
		pairedBytes: '0',
		baseWork: '268435456',
		noiseWork: '0',
	};
}
const view = () => ({
	kind: 'view',
	protocol: 8,
	id: 2,
	...identity(),
	token: '1',
	generation: '7',
	visible: [1, 1, 130, 66],
	level: 1,
});
const opened = () => ({
	kind: 'opened',
	protocol: 8,
	id: 1,
	...identity(),
	byteLength: '4096',
	canvas: [130, 66],
	storedNonNormal: true,
	storedNoise: false,
});
const meta = (kind, id = 2) => ({
	kind,
	protocol: 8,
	id,
	...identity(),
	token: '1',
	generation: '7',
});
function patch(ordinal = 0) {
	return {
		...meta('patch'),
		ordinal,
		patch: ordinal === 0 ? [0, 0, 64, 33] : [64, 0, 65, 33],
		rgba: new ArrayBuffer(ordinal === 0 ? 8448 : 132),
	};
}
const ack = (ordinal = 0, id = 3) => ({
	...meta('ack', id),
	ordinal,
	patch: patch(ordinal).patch,
});
const complete = () => ({ ...meta('complete'), patchCount: 2 });
const close = (id = 5, token = '1') => ({
	kind: 'close',
	protocol: 8,
	id,
	...identity(),
	token,
});
const reject = (f, code) => assert.throws(f, (e) => e.code === code);

test('strict compiled READY: every revision, cap and actual control getter must match', () => {
	// Fixture controls are independent test metadata, never product quote values.
	const facts = {
		...VERSIONS8,
		limits: { ...LIMITS8 },
		controls: {
			serviceBytes: '4096',
			stageBytes: '112',
			viewBytes: '4096',
		},
	};
	assert.ok(
		Object.isFrozen(
			validateReady8({ kind: 'ready', runtimeFacts: facts }, facts),
		),
	);
	for (const key of Object.keys(VERSIONS8)) {
		const bad = structuredClone(facts);
		bad[key]++;
		reject(
			() => validateReady8({ kind: 'ready', runtimeFacts: bad }, facts),
			'CapabilityUnsupported',
		);
	}
	for (const key of Object.keys(LIMITS8)) {
		const bad = structuredClone(facts);
		bad.limits[key] = typeof bad.limits[key] === 'string' ? '1' : 1;
		reject(
			() => validateReady8({ kind: 'ready', runtimeFacts: bad }, facts),
			'CapabilityUnsupported',
		);
	}
	const bad = structuredClone(facts);
	bad.controls.stageBytes = '113';
	reject(
		() => validateReady8({ kind: 'ready', runtimeFacts: bad }, facts),
		'CapabilityUnsupported',
	);
});

test('canonical integers, exact own fields and data descriptors reject hostile metadata', () => {
	for (const n of [
		'00',
		'+1',
		'-1',
		' 1',
		'1.0',
		'1e2',
		'18446744073709551616',
		1,
		null,
	])
		reject(() => decimal8(n), 'InputSchema');
	assert.equal(decimal8('18446744073709551615'), 18446744073709551615n);
	for (const change of [
		(v) => {
			v.extra = 1;
		},
		(v) => {
			v[Symbol('x')] = 1;
		},
		(v) => {
			Object.defineProperty(v, 'x', { value: 1 });
		},
		(v) => {
			Object.setPrototypeOf(v, { inherited: true });
		},
		(v) => {
			Object.defineProperty(v, 'owner', {
				get() {
					assert.fail('getter ran');
				},
				enumerable: true,
			});
		},
		(v) => {
			v.id = -0;
		},
		(v) => {
			v.id = 4294967296;
		},
		(v) => {
			v.sourceSha256 = 'A'.repeat(64);
		},
		(v) => {
			v.owner = uuid('f').toUpperCase();
		},
	]) {
		const bad = open();
		change(bad);
		reject(() => validateOpen8(bad), 'InputSchema');
	}
	const inherited = open();
	Object.setPrototypeOf(inherited.canvas, {
		toJSON() {
			assert.fail('inherited toJSON ran');
		},
	});
	reject(() => validateOpen8(inherited), 'InputSchema');
	const inheritedRect = view();
	Object.setPrototypeOf(inheritedRect.visible, {
		toJSON() {
			assert.fail('rectangle toJSON ran');
		},
	});
	reject(() => validateView8(inheritedRect, open()), 'InputSchema');
	const nonenumerableOpen = open(),
		nonenumerableReply = opened();
	nonenumerableReply.canvas = [129, 65];
	for (const v of [nonenumerableOpen.canvas, nonenumerableReply.canvas])
		for (const k of ['0', '1'])
			Object.defineProperty(v, k, { enumerable: false });
	reject(
		() => validateOpened8(nonenumerableReply, nonenumerableOpen),
		'InputSchema',
	);
	reject(() => validateOpened8(nonenumerableReply, open()), 'InputSchema');
	const hiddenRect = view();
	Object.defineProperty(hiddenRect.visible, '0', { enumerable: false });
	reject(() => validateView8(hiddenRect, open()), 'InputSchema');
	const getterRect = view();
	Object.defineProperty(getterRect.visible, '0', {
		get() {
			assert.fail('slot getter ran');
		},
		enumerable: true,
	});
	reject(() => validateView8(getterRect, open()), 'InputSchema');
});

test('OPEN keeps distinct public/private state and refuses route, role and split ceiling violations', () => {
	const value = open(),
		frozen = validateOpen8(value);
	assert.notEqual(frozen.publicTicket.state, frozen.sourceState);
	value.publicTicket.epoch = '20';
	assert.equal(frozen.publicTicket.epoch, '19');
	for (const [change, code] of [
		[
			(v) => {
				v.storedNonNormal = false;
			},
			'CapabilityUnsupported',
		],
		[
			(v) => {
				v.storedNoise = true;
			},
			'CapabilityUnsupported',
		],
		[
			(v) => {
				v.roles.E = '4095';
			},
			'ResourceLimit',
		],
		[
			(v) => {
				v.roles.W = '67108864';
			},
			'ResourceLimit',
		],
		[
			(v) => {
				v.pairedBytes = '67108864';
			},
			'ResourceLimit',
		],
		[
			(v) => {
				v.baseWork = '268435457';
			},
			'ResourceLimit',
		],
		[
			(v) => {
				v.noiseWork = '1';
			},
			'ResourceLimit',
		],
		[
			(v) => {
				v.byteLength = '67108865';
			},
			'ResourceLimit',
		],
	]) {
		const bad = open();
		change(bad);
		reject(() => validateOpen8(bad), code);
	}
	const noise = open();
	noise.mode = 'noise';
	noise.storedNoise = true;
	noise.noiseWork = '34359738368';
	assert.equal(validateOpen8(noise).noiseWork, '34359738368');
	noise.noiseWork = '34359738369';
	reject(() => validateOpen8(noise), 'ResourceLimit');
	const exact = open();
	for (const role of Object.keys(exact.roles)) exact.roles[role] = '0';
	exact.roles.E = '4096';
	exact.roles.W = '67104768'; // exact64MiB metadata boundary, not a usable Rust grant
	validateOpen8(exact);
	exact.roles.W = '67104769';
	reject(() => validateOpen8(exact), 'ResourceLimit');
});

test('source bytes require exact fixed ArrayBuffer without glue Vec assumption', () => {
	validateSourceBuffer8(new ArrayBuffer(4096), open());
	for (const value of [
		new ArrayBuffer(4095),
		new Uint8Array(4096),
		new SharedArrayBuffer(4096),
	])
		reject(() => validateSourceBuffer8(value, open()), 'InputSchema');
	const detached = new ArrayBuffer(4096);
	structuredClone(detached, { transfer: [detached] });
	reject(() => validateSourceBuffer8(detached, open()), 'InputSchema');
	if (typeof ArrayBuffer.prototype.resize === 'function')
		reject(
			() =>
				validateSourceBuffer8(
					new ArrayBuffer(4096, { maxByteLength: 8192 }),
					open(),
				),
			'InputSchema',
		);
});

test('global power2 grid, partial edge and row-major64 follow independently frozen geometry', () => {
	assert.deepEqual(geometry8([1, 1, 130, 66], 1, [130, 66]), {
		region: [0, 0, 130, 66],
		size: [65, 33],
		patches: 2,
	});
	assert.deepEqual(patchRect8(view(), open().canvas, 0), [0, 0, 64, 33]);
	assert.deepEqual(patchRect8(view(), open().canvas, 1), [64, 0, 65, 33]);
	const four = { ...view(), visible: [0, 0, 130, 130], level: 0 };
	assert.deepEqual(patchRect8(four, [130, 130], 3), [0, 64, 64, 128]);
	for (const [visible, level] of [
		[[0, 0, NaN, 66], 1],
		[[0, 0, 131, 66], 1],
		[[1.5, 0, 130, 66], 1],
		[[0, 0, 130, 66], 21],
		[[0, 0, 130, 66], Infinity],
	])
		reject(() => geometry8(visible, level, [130, 66]), 'InputSchema');
	reject(
		() => geometry8([0, 0, 1048576, 1048576], 0, [1048576, 1048576]),
		'ResourceLimit',
	);
});

test('patch bytes and complete identities bind to original VIEW rather than current mutable objects', () => {
	validatePatch8(patch(), view(), open(), 0);
	validateAck8(ack(), view(), open(), 0);
	validateComplete8(complete(), view(), open());
	for (const key of [
		'owner',
		'document',
		'sourceState',
		'draft',
		'sourceSha256',
		'profileSha256',
		'publicTicket',
	]) {
		const bad = patch();
		bad[key] =
			key === 'publicTicket'
				? { state: uuid('8'), epoch: '19' }
				: uuid('8');
		assert.throws(() => validatePatch8(bad, view(), open(), 0));
	}
	for (const change of [
		(v) => {
			v.rgba = new ArrayBuffer(8447);
		},
		(v) => {
			v.ordinal = 1;
		},
		(v) => {
			v.patch = [1, 0, 65, 33];
		},
		(v) => {
			v.id = 3;
		},
		(v) => {
			v.token = '2';
		},
		(v) => {
			v.generation = '8';
		},
	]) {
		const bad = patch();
		change(bad);
		assert.throws(() => validatePatch8(bad, view(), open(), 0));
	}
	const bad = view();
	bad.publicTicket.state = bad.sourceState;
	reject(() => validateView8(bad, open()), 'ConflictStaleBase');
});

test('one patch outstanding; exact ACK permits next row-major patch; complete only after all copies', () => {
	const s = new Protocol8Sequence();
	s.open(open());
	reject(() => s.view(view()), 'ConflictStaleBase');
	s.opened(opened());
	s.view(view());
	reject(() => s.complete(complete()), 'ConflictStaleBase');
	reject(() => s.ack(ack()), 'ConflictStaleBase');
	s.patch(patch());
	reject(() => s.patch(patch()), 'ConflictStaleBase');
	reject(() => s.complete(complete()), 'ConflictStaleBase');
	s.ack(ack());
	reject(() => s.ack(ack()), 'ConflictStaleBase');
	s.patch(patch(1));
	s.ack(ack(1, 4));
	s.complete(complete());
	reject(() => s.patch(patch()), 'ConflictStaleBase');
	reject(() => s.view({ ...view(), id: 5 }), 'ConflictStaleBase');
	s.view({ ...view(), id: 5, token: '2', generation: '8' }); // same geometry, explicit new operation
});

test('cancelled operation never renews; close binding is distinct from evidence of joined credit', () => {
	const s = new Protocol8Sequence();
	reject(() => s.view(view()), 'ConflictStaleBase');
	s.open(open());
	reject(() => s.open({ ...open(), id: 2 }), 'ConflictStaleBase');
	reject(
		() => s.opened({ ...opened(), sourceState: uuid('9') }),
		'ConflictStaleBase',
	);
	s.opened(opened());
	s.view(view());
	s.patch(patch());
	s.cancel(meta('cancel', 3));
	reject(() => s.patch(patch()), 'ConflictStaleBase');
	reject(() => s.view({ ...view(), id: 4 }), 'ConflictStaleBase');
	reject(
		() => s.view({ ...view(), id: 4, token: '2', generation: '8' }),
		'ConflictStaleBase',
	);
	reject(() => s.cancelled(meta('cancelled', 4)), 'ConflictStaleBase');
	s.cancelled(meta('cancelled', 3));
	s.view({ ...view(), id: 4, token: '2', generation: '8' });
	reject(() => s.close(close(5, '1')), 'ConflictStaleBase');
	s.close(close(5, '2'));
	const closed = { ...close(5, '2'), kind: 'closed' };
	reject(() => s.closed({ ...closed, token: '1' }), 'ConflictStaleBase');
	validateClosed8(closed, close(5, '2'), open());
	s.closed(closed);
	reject(() => s.closed(closed), 'ConflictStaleBase');
	reject(
		() => s.view({ ...view(), id: 6, token: '3', generation: '9' }),
		'ConflictStaleBase',
	);
});
