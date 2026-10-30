import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';
import {
	canonical,
	sha,
	validateBinding,
	validatePlan,
	validateRequest,
	validateResult,
} from '../src/lib/filter-preview-protocol.js';

test('uncertain active failure rejects the latest local job that was never posted', async () => {
	const { client, started } = await errorClient(
		'update',
		false,
		'CodecFailed',
		true,
	);
	const first = client
		.base({ generation: '1', view: request.view })
		.catch((e) => e);
	await started;
	let latestRejected = false;
	const latest = client.base({ generation: '2', view: request.view });
	latest.catch(() => {
		latestRejected = true;
	});
	assert.equal((await first).code, 'CodecFailed');
	await Promise.resolve();
	assert.equal(latestRejected, true);
});
test('unrecognized error cannot mint reusable completion with only a flag', async () => {
	const { service, client } = await errorClient(
		'update',
		true,
		'invented.completion',
	);
	await assert.rejects(client.base({ generation: '1', view: request.view }));
	assert.equal(service.filterPreviewUsage().quarantined, true);
	assert.ok(service.filterPreviewUsage().reserved > 0);
	await assert.rejects(client.close(), (e) => e.code === 'CodecFailed');
});

async function errorClient(
	failingKind,
	completed = false,
	code = completed ? 'resource.limit' : 'CodecFailed',
	delayed = false,
) {
	let notify;
	const started = new Promise((resolve) => {
		notify = resolve;
	});
	class ErrorWorker {
		postMessage(message) {
			queueMicrotask(async () => {
				if (message.kind === 'open') {
					this.plan = message.plan;
					this.onmessage({
						data: {
							kind: 'ready',
							id: message.id,
							binding: this.plan,
						},
					});
					return;
				}
				if (message.kind === failingKind && !this.failed) {
					this.failed = true;
					notify();
					const emit = () =>
						this.onmessage({
							data: {
								kind: 'error',
								id: message.id,
								code,
								message: 'injected failure',
								completed,
							},
						});
					if (delayed) setTimeout(emit, 20);
					else emit();
					return;
				}
				if (message.kind === 'cancel') {
					this.onmessage({
						data: {
							kind: 'retired',
							id: message.id,
							generation: message.generation,
						},
					});
					return;
				}
				if (message.kind === 'close') {
					this.onmessage({
						data: { kind: 'closed', id: message.id },
					});
					return;
				}
				const r = message.request;
				this.onmessage({
					data: {
						...frame(),
						id: message.id,
						generation: r.generation,
						owner: this.plan.owner,
						sourceSha256: this.plan.sourceSha256,
						operationSha256: await sha(
							new TextEncoder().encode(canonical(r.operation)),
						),
					},
				});
			});
		}
		terminate() {}
	}
	globalThis.Worker = ErrorWorker;
	const service = await import(
		`../src/lib/filter-preview-worker.js?error=${failingKind}-${completed}-${Date.now()}`
	);
	const source = new Uint8Array([10, 11, 12]);
	return {
		service,
		started,
		client: await service
			.beginFilterPreview(plan)
			.open(source, await sha(source)),
	};
}
for (const kind of ['update', 'cancel', 'close']) {
	test(`unknown matching ${kind} error quarantines without an invented retirement acknowledgment`, async () => {
		const { service, client } = await errorClient(kind);
		const call =
			kind === 'update'
				? client.base({ generation: '1', view: request.view })
				: kind === 'cancel'
					? client.cancel('1')
					: client.close();
		await assert.rejects(call);
		assert.deepEqual(service.filterPreviewUsage().quarantined, true);
		assert.ok(service.filterPreviewUsage().reserved > 0);
		await assert.rejects(client.close(), (e) => e.code === 'CodecFailed');
		await assert.rejects(
			client.base({ generation: '2', view: request.view }),
			(e) => e.code === 'CodecFailed',
		);
		assert.throws(
			() => service.beginFilterPreview(plan),
			(e) => e.code === 'ResourceLimit',
		);
	});
}
test('completed typed UPDATE refusal retires its job but keeps the exact source reusable', async () => {
	const { service, client } = await errorClient('update', true);
	await assert.rejects(
		client.base({ generation: '1', view: request.view }),
		(e) => e.code === 'resource.limit',
	);
	assert.equal(service.filterPreviewUsage().quarantined, false);
	const output = await client.base({ generation: '2', view: request.view });
	output.release();
	const [ack, concurrent] = await Promise.all([
		client.close(),
		client.close(),
	]);
	assert.deepEqual(concurrent, ack);
	assert.ok(Object.isFrozen(ack));
	assert.throws(() => {
		ack.kind = 'forged';
	}, TypeError);
	assert.deepEqual(await client.close(), ack);
	assert.equal(service.filterPreviewUsage().reserved, 0);
});
test('completed typed OPEN refusal releases its exact lease while unknown failure quarantines', async () => {
	const source = new Uint8Array([7, 8, 9]),
		sourceSha = await sha(source);
	for (const completed of [true, false]) {
		class RefusalWorker {
			postMessage(message) {
				queueMicrotask(() =>
					this.onmessage({
						data: {
							kind: 'error',
							id: message.id,
							code: completed ? 'resource.limit' : 'CodecFailed',
							message: 'refused',
							completed,
						},
					}),
				);
			}
			terminate() {}
		}
		globalThis.Worker = RefusalWorker;
		const service = await import(
			`../src/lib/filter-preview-worker.js?refusal=${completed}-${Date.now()}`
		);
		await assert.rejects(
			service.beginFilterPreview(plan).open(source, sourceSha),
		);
		assert.equal(service.filterPreviewUsage().quarantined, !completed);
		if (completed) {
			assert.equal(service.filterPreviewUsage().reserved, 0);
			service.beginFilterPreview(plan).cancel();
		} else assert.ok(service.filterPreviewUsage().reserved > 0);
	}
});

globalThis.crypto ??= webcrypto;
const uuid = '00000000-0000-4000-8000-000000000001';
const plan = {
	owner: { session: uuid, state: uuid, epoch: '0', frontend: 3 },
	draft: uuid,
	document: uuid,
	size: [3, 2],
	layers: 1,
	residentBytes: 262144,
	metadataBytes: 0,
	metadataCharge: 0,
	maskCount: 0,
	maskBands: 0,
	maskRuns: 0,
	maskDecodeBytes: 0,
	filterDigest: '0'.repeat(64),
	sourceSha256: '1'.repeat(64),
};
const request = {
	generation: '1',
	operation: null,
	target: null,
	view: { region: [0, 0, 3, 2], level: 0 },
	render: true,
};
test('pre-Worker normalization failure cannot retain the admission lease', async () => {
	const service = await import(
		`../src/lib/filter-preview-worker.js?normalization=${Date.now()}`
	);
	assert.throws(
		() => service.beginFilterPreview({ ...plan, sourceSha256: undefined }),
		(e) => e.code === 'InputSchema',
	);
	assert.deepEqual(service.filterPreviewUsage(), {
		reserved: 0,
		quarantined: false,
	});
	const admission = service.beginFilterPreview(plan);
	admission.cancel();
	assert.equal(service.filterPreviewUsage().reserved, 0);
});
function frame() {
	return {
		kind: 'frame',
		id: 1,
		generation: '1',
		owner: plan.owner,
		draft: uuid,
		sourceSha256: plan.sourceSha256,
		target: null,
		view: request.view,
		width: 3,
		height: 2,
		region: [0, 0, 3, 2],
		level: 0,
		operationSha256: '2'.repeat(64),
		pixels: new Uint8Array(24),
	};
}
test('source and exact aligned frame controls accept canonical sparse dimensions', () => {
	validatePlan(plan);
	validateBinding(plan, plan);
	validateRequest(request, plan);
	validateResult(
		frame(),
		{ ...request, operationSha256: '2'.repeat(64) },
		plan,
	);
	validatePlan({ ...plan, size: [1_048_576, 1], residentBytes: 0 });
});
test('malformed/deep owner, unknown fields and unsafe integers refuse before recursive normalization', () => {
	let deep = {};
	for (let i = 0; i < 10000; i++) deep = { child: deep };
	for (const p of [
		{ ...plan, owner: deep },
		{ ...plan, layers: Number.MAX_SAFE_INTEGER + 1 },
		{ ...plan, extra: true },
		{ ...plan, residentBytes: 1 },
		{ ...plan, owner: { ...plan.owner, epoch: '00' } },
	])
		assert.throws(
			() => validatePlan(p),
			(e) => e.code === 'InputSchema',
		);
	assert.throws(
		() => canonical(deep),
		(e) => e.code === 'InputSchema',
	);
	assert.throws(
		() => canonical({ a: 'x'.repeat(4097) }),
		(e) => e.code === 'InputSchema',
	);
	assert.throws(
		() => validateRequest({ ...request, view: deep }, plan),
		(e) => e.code === 'InputSchema',
	);
	assert.throws(
		() => validateResult({ ...frame(), owner: deep }, request, plan),
		(e) => e.code === 'InputSchema',
	);
});
test('forged source/operation/generation/ROI and byte extents refuse independently', () => {
	const expected = { ...request, operationSha256: '2'.repeat(64) };
	for (const change of [
		{ sourceSha256: '3'.repeat(64) },
		{ operationSha256: '3'.repeat(64) },
		{ generation: '2' },
		{ region: [1, 0, 3, 2] },
		{ pixels: new Uint8Array(23) },
		{ view: { region: [0, 0, 4, 2], level: 0 } },
		{ width: 4 },
		{ target: uuid },
	])
		assert.throws(() =>
			validateResult({ ...frame(), ...change }, expected, plan),
		);
});
test('canonical operation digest is sorted and bounded before hashing', async () => {
	assert.equal(canonical({ z: false, a: 2 }), ' {"a":2,"z":false}'.trim());
	assert.equal(
		await sha(new TextEncoder().encode(canonical(null))),
		'74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b',
	);
});
test('persistent client keeps one source across ordinary retirement and requires frame consumption', async () => {
	let opens = 0,
		closed = 0,
		updates = 0;
	class FakeWorker {
		postMessage(message) {
			queueMicrotask(async () => {
				let reply;
				if (message.kind === 'open') {
					opens++;
					this.plan = message.plan;
					reply = {
						kind: 'ready',
						id: message.id,
						binding: this.plan,
					};
				} else if (message.kind === 'close') {
					closed++;
					reply = { kind: 'closed', id: message.id };
				} else if (message.kind === 'cancel')
					reply = {
						kind: 'retired',
						id: message.id,
						generation: message.generation,
					};
				else {
					updates++;
					const r = message.request;
					reply = {
						...frame(),
						id: message.id,
						kind: r.render ? 'frame' : 'prepared',
						generation: r.generation,
						owner: this.plan.owner,
						sourceSha256: this.plan.sourceSha256,
						target: r.target,
						view: r.view,
						operationSha256: await sha(
							new TextEncoder().encode(canonical(r.operation)),
						),
					};
					if (!r.render) delete reply.pixels;
				}
				this.onmessage({ data: reply });
			});
		}
		terminate() {}
	}
	globalThis.Worker = FakeWorker;
	const service = await import(
		`../src/lib/filter-preview-worker.js?client-test=${Date.now()}`
	);
	const admission = service.beginFilterPreview(plan);
	assert.throws(
		() => service.beginFilterPreview(plan),
		(e) => e.code === 'ResourceLimit',
	);
	const bytes = new Uint8Array([1, 2, 3]),
		sourceSha = await sha(bytes);
	const client = await admission.open(bytes, sourceSha);
	assert.equal(opens, 1);
	assert.equal(client.binding.sourceSha256, sourceSha);
	const first = await client.base({ generation: '1', view: request.view });
	await assert.rejects(
		client.base({ generation: '2', view: request.view }),
		(e) => e.code === 'ResourceLimit',
	);
	first.release();
	assert.equal((await client.cancel('1')).kind, 'retired');
	assert.equal(service.filterPreviewUsage().quarantined, false);
	const prepared = await client.update({
		...request,
		generation: '2',
		render: false,
	});
	prepared.release();
	const second = await client.base({ generation: '3', view: request.view });
	second.release();
	assert.equal((await client.close()).kind, 'closed');
	assert.equal(closed, 1);
	assert.equal(opens, 1);
	assert.equal(updates, 3);
	assert.equal(service.filterPreviewUsage().reserved, 0);
});
test('slider burst retains only active plus latest and acknowledges retirement without source reload', async () => {
	let opens = 0,
		updates = 0,
		cancels = 0;
	class SlowWorker {
		postMessage(message) {
			const emit = (data) =>
				queueMicrotask(() => this.onmessage({ data }));
			if (message.kind === 'open') {
				opens++;
				this.plan = message.plan;
				emit({ kind: 'ready', id: message.id, binding: this.plan });
			} else if (message.kind === 'close')
				emit({ kind: 'closed', id: message.id });
			else if (message.kind === 'cancel') {
				cancels++;
				if (this.active) {
					emit({
						kind: 'retired',
						id: this.active.id,
						generation: this.active.request.generation,
					});
					this.active = null;
				}
				emit({
					kind: 'retired',
					id: message.id,
					generation: message.generation,
				});
			} else {
				updates++;
				this.active = message;
				setTimeout(async () => {
					if (this.active !== message) return;
					this.active = null;
					const r = message.request;
					emit({
						...frame(),
						id: message.id,
						generation: r.generation,
						sourceSha256: this.plan.sourceSha256,
						operationSha256: await sha(
							new TextEncoder().encode(canonical(r.operation)),
						),
					});
				}, 30);
			}
		}
		terminate() {}
	}
	globalThis.Worker = SlowWorker;
	const service = await import(
		`../src/lib/filter-preview-worker.js?burst=${Date.now()}`
	);
	const source = new Uint8Array([4, 5, 6]);
	const admission = service.beginFilterPreview(plan);
	const client = await admission.open(source, await sha(source));
	const initial = client
		.base({ generation: '1', view: request.view })
		.catch((e) => e);
	await new Promise((resolve) => setTimeout(resolve, 8));
	const burst = [];
	for (let n = 2; n <= 1001; n++)
		burst.push(
			client
				.base({ generation: String(n), view: request.view })
				.catch((e) => e),
		);
	const result = await Promise.all(burst);
	assert.equal((await initial).code, 'OperationCancelled');
	assert.equal(
		result.filter((x) => x.code === 'OperationCancelled').length,
		999,
	);
	assert.equal(result.at(-1).generation, '1001');
	result.at(-1).release();
	assert.equal(updates, 2);
	assert.equal(opens, 1);
	assert.equal(cancels, 1);
	assert.equal(service.filterPreviewUsage().quarantined, false);
	await client.close();
	assert.equal(service.filterPreviewUsage().reserved, 0);
});

test('protocol2 mask facts price expanded instances and old source plans refuse', () => {
	const old = { ...plan };
	for (const key of ['maskCount', 'maskBands', 'maskRuns', 'maskDecodeBytes'])
		delete old[key];
	assert.throws(() => validatePlan(old));
	validatePlan({ ...plan, maskCount: 1 }); // stored zero mask has no heap payload
	const masked = {
		...plan,
		maskCount: 2,
		maskBands: 2,
		maskRuns: 3,
		maskDecodeBytes: 1024,
	};
	validatePlan(masked);
	for (const delta of [
		{ maskCount: 0 },
		{ maskBands: 0 },
		{ maskRuns: 0 },
		{ maskDecodeBytes: 0 },
		{ maskDecodeBytes: -1 },
		{ maskDecodeBytes: Number.MAX_SAFE_INTEGER },
		{ maskBands: 262145 },
	])
		assert.throws(() => validatePlan({ ...masked, ...delta }));
	for (const key of [
		'maskCount',
		'maskBands',
		'maskRuns',
		'maskDecodeBytes',
	]) {
		const binding = { ...masked, [key]: masked[key] + 1 };
		assert.throws(() => validateBinding(binding, masked));
	}
});
