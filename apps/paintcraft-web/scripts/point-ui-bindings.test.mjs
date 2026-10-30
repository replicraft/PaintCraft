import assert from 'node:assert/strict';
import test from 'node:test';
import { matchingPointPair } from './lib/point-ui-bindings.mjs';

function specimen(base = false) {
	const owner = {
		session: '44444444-4444-4444-8444-444444444444',
		state: '11111111-1111-4111-8111-111111111111',
		epoch: '9',
		frontend: 3,
	};
	const binding = {
		workerId: 1,
		owner,
		draft: '55555555-5555-4555-8555-555555555555',
		sourceSha256: 'ab'.repeat(32),
	};
	const operation = base
		? null
		: {
				op: 'filter.add',
				layer_ref: { id: '22222222-2222-4222-8222-222222222222' },
				ref: 'studio_filter',
				descriptor: { kind: 'threshold-luma-v1', level: 100 },
				enabled: true,
			};
	const expected = {
		binding,
		operation,
		operationSha256: base ? 'cd'.repeat(32) : 'ef'.repeat(32),
		generation: '4',
		pixels: [0, 0, 0, 255, 255, 255, 255, 255],
		since: 2,
		target: base ? null : '22222222-2222-4222-8222-222222222222',
	};
	const view = { region: [0, 0, 2, 1], level: 0 };
	const request = {
		...binding,
		generation: '4',
		target: expected.target,
		operation,
		view,
		render: true,
	};
	const reply = {
		...binding,
		sequence: 4,
		workerId: 1,
		kind: 'frame',
		generation: '4',
		target: expected.target,
		operationSha256: expected.operationSha256,
		view,
		region: [0, 0, 2, 1],
		level: 0,
		width: 2,
		height: 1,
		pixels: expected.pixels,
	};
	return { expected, events: [{ sequence: 3, workerId: 1, request }, reply] };
}

test('one exact pair predicate binds all independent context, operation and full-plane fields', () => {
	for (const base of [false, true]) {
		const { expected, events } = specimen(base);
		assert.ok(matchingPointPair(events, expected));
		for (const mutate of [
			(e) => {
				e[0].workerId = 2;
				e[1].workerId = 2;
			},
			(e) => {
				e[1].sourceSha256 = 'ff'.repeat(32);
			},
			(e) => {
				e[0].request.sourceSha256 = 'ff'.repeat(32);
				e[1].sourceSha256 = 'ff'.repeat(32);
			},
			(e) => {
				e[0].request.owner.state = 'foreign';
				e[1].owner.state = 'foreign';
			},
			(e) => {
				e[0].request.owner.epoch = '10';
				e[1].owner.epoch = '10';
			},
			(e) => {
				e[0].request.owner.session = 'foreign';
				e[1].owner.session = 'foreign';
			},
			(e) => {
				e[0].request.draft = 'foreign';
				e[1].draft = 'foreign';
			},
			(e) => {
				e[1].generation = '5';
			},
			(e) => {
				e[0].request.generation = '5';
				e[1].generation = '5';
			},
			(e) => {
				e[1].operationSha256 = '00'.repeat(32);
			},
			(e) => {
				e[0].request.operation = { op: 'filter.mask.remove' };
			},
			(e) => {
				e[0].request.target = 'foreign';
				e[1].target = 'foreign';
			},
			(e) => {
				e[0].request.view.region = [1, 0, 1, 1];
				e[1].view.region = [1, 0, 1, 1];
			},
			(e) => {
				e[1].region = [1, 0, 1, 1];
			},
			(e) => {
				e[1].level = 1;
			},
			(e) => {
				e[1].width = 1;
			},
			(e) => {
				e[1].pixels[0] = 1;
			},
			(e) => {
				e[0].sequence = 2;
			},
			(e) => {
				e[0].request.render = false;
			},
		]) {
			const forged = structuredClone(events);
			mutate(forged);
			assert.equal(matchingPointPair(forged, expected), null);
		}
	}
});

test('BASE cannot match an earlier private frame with coincidentally identical pixels', () => {
	const base = specimen(true),
		privateFrame = specimen(false);
	assert.equal(matchingPointPair(privateFrame.events, base.expected), null);
	const combined = [
		...privateFrame.events,
		...base.events.map((e) => ({ ...e, sequence: e.sequence + 2 })),
	];
	assert.deepEqual(
		matchingPointPair(combined, base.expected)?.request.operation,
		null,
	);
	const unrelated = structuredClone(privateFrame.events[1]);
	unrelated.sequence = 99;
	assert.equal(
		matchingPointPair([...base.events, unrelated], base.expected).reply
			.target,
		null,
	);
});
