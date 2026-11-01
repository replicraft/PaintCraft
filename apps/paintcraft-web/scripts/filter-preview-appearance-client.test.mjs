import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';
import { createLayerAppearanceController7 } from '../src/lib/filter-preview-appearance-client.js';
import { createRenderAdmission } from '../src/lib/render-admission.js';
import {
	admissionDigest7,
	numericalDigests7,
	numericalEntry7,
} from '../src/lib/filter-preview-appearance-protocol.js';
import { RESERVATION_BYTES, sha } from '../src/lib/filter-preview-protocol.js';
import { encodeProgram } from '../src/lib/filter-preview-program-protocol.js';
import {
	binding,
	begin,
	draft,
	owner,
	plan,
	runtime,
} from './filter-preview-appearance-client-fixtures.mjs';
globalThis.crypto ??= webcrypto;
const preplan = {
	sourceProtocol: 7,
	registryContract: 3,
	mode: 'base',
	owner,
	draft,
};
const deferred = () => {
	let resolve;
	const promise = new Promise((r) => {
		resolve = r;
	});
	return { promise, resolve };
};
class MockWorker {
	messages = [];
	waiters = [];
	terminated = false;
	plan = null;
	handler = null;
	postMessage(input, transfer) {
		const message = structuredClone(input, { transfer });
		this.messages.push(message);
		for (const waiter of this.waiters.splice(0)) waiter();
		Promise.resolve()
			.then(() =>
				this.handler ? this.handler(message) : this.normal(message),
			)
			.catch((error) => {
				this.error = error;
				this.onerror?.({ error });
			});
	}
	reply(message, value) {
		this.onmessage({ data: { ...value, id: message.id } });
	}
	async normal(message) {
		switch (message.kind) {
			case 'runtime-ready':
				return this.reply(message, {
					kind: 'runtime-ready',
					runtimeFacts: runtime(message.noise),
					controlQuote: runtime().controlQuoteBytes,
				});
			case 'hello': {
				const entries = message.pairs.map(numericalEntry7),
					numerical = await numericalDigests7(message.pairs, entries);
				return this.reply(message, {
					kind: 'capabilities',
					protocol: 7,
					registryContract: 3,
					entries,
					appearance: message.appearance,
					admissionSha256: await admissionDigest7(message.admission),
					requiredSha256: numerical.requiredSha256,
					resolutionSha256: numerical.resolutionSha256,
					runtimeFacts: runtime(message.admission.mode === 'noise'),
				});
			}
			case 'open':
				this.plan = message.plan;
				return this.reply(message, {
					kind: 'ready',
					binding: message.plan,
				});
			case 'update': {
				const out = await binding(this.plan, message.request);
				return this.reply(
					message,
					message.request.render
						? {
								...out,
								pixels: new Uint8Array(
									out.width * out.height * 4,
								),
							}
						: out,
				);
			}
			case 'cancel':
				return this.reply(message, {
					kind: 'retired',
					generation: message.generation,
				});
			case 'close':
				return this.reply(message, { kind: 'closed' });
			default:
				throw new Error(`Unexpected mock kind ${message.kind}`);
		}
	}
	terminate() {
		this.terminated = true;
	}
	async wait(kind, count = 1) {
		while (this.messages.filter((m) => m.kind === kind).length < count)
			await new Promise((resolve) => this.waiters.push(resolve));
		return this.messages.filter((m) => m.kind === kind)[count - 1];
	}
}
function setup(timeout) {
	const broker = createRenderAdmission(),
		worker = new MockWorker();
	const controller = createLayerAppearanceController7(
		broker,
		() => worker,
		timeout,
	);
	return { broker, worker, controller };
}
async function prepared(env) {
	const admission = env.controller.beginLayerAppearancePreview(preplan);
	await admission.ready;
	const guard = admission.reserveMain(4096),
		p = await plan();
	const hello = {
		protocol: 7,
		registryContract: 3,
		pairs: p.sourceRequirements.pairs,
		appearance: p.appearance,
		admission: p.admission,
		slice: p.slice,
	};
	return { admission, guard, p, hello };
}
async function opened(env) {
	const context = await prepared(env),
		source = new Uint8Array([1, 2, 3]);
	const promise = context.admission.open(source, context.p.sourceSha256, {
		plan: context.p,
		hello: context.hello,
	});
	assert.equal(source.byteLength, 0);
	return { ...context, client: await promise };
}
test('Source7 ready/prelease and closed ACK retain one shared slot until independent Main release', async () => {
	const env = setup(),
		{ admission, guard } = await prepared(env);
	assert.equal(env.broker.usage().reserved, RESERVATION_BYTES + 1048576);
	assert.throws(() => env.controller.beginLayerAppearancePreview(preplan), {
		code: 'ResourceLimit',
	});
	assert.deepEqual(await admission.close(), { kind: 'closed' });
	assert.notEqual(env.broker.usage().reserved, 0);
	guard.release();
	guard.release();
	assert.equal(env.broker.usage().reserved, 0);
});
test('Source7 startup close joins Worker even when ready is suppressed', async () => {
	const env = setup();
	env.worker.handler = (m) =>
		m.kind === 'close' ? env.worker.normal(m) : undefined;
	const admission = env.controller.beginLayerAppearancePreview(preplan);
	const rejected = assert.rejects(admission.ready, {
		code: 'OperationCancelled',
	});
	await admission.close();
	await rejected;
	assert.equal(env.broker.usage().reserved, 0);
	assert.equal(env.broker.usage().quarantined, false);
});
test('Source7 exclusive source is detached before HELLO awaits and no source hashing occurs', async () => {
	const env = setup(),
		{ admission, guard, p, hello } = await prepared(env);
	const original = env.worker.normal.bind(env.worker),
		gate = deferred();
	env.worker.handler = async (m) => {
		if (m.kind === 'hello') await gate.promise;
		return original(m);
	};
	const source = new Uint8Array([9, 7]),
		opening = admission.open(source, p.sourceSha256, { plan: p, hello });
	assert.equal(source.byteLength, 0);
	await env.worker.wait('hello');
	assert.equal(
		env.worker.messages.some((m) => m.kind === 'open'),
		false,
	);
	gate.resolve();
	const client = await opening;
	assert.deepEqual(
		[
			...new Uint8Array(
				env.worker.messages.find((m) => m.kind === 'open').source,
			),
		],
		[9, 7],
	);
	await client.close();
	guard.release();
});
test('Source7 rejects partial/shared/source SHA mismatches without source adoption', async () => {
	for (const choice of ['offset', 'sha', 'shared']) {
		const env = setup(),
			ctx = await prepared(env);
		const bytes =
			choice === 'offset'
				? new Uint8Array(new ArrayBuffer(4), 1, 2)
				: choice === 'shared'
					? new Uint8Array(new SharedArrayBuffer(2))
					: new Uint8Array([1]);
		await assert.rejects(
			ctx.admission.open(
				bytes,
				choice === 'sha' ? '00'.repeat(32) : ctx.p.sourceSha256,
				{ plan: ctx.p, hello: ctx.hello },
			),
		);
		await ctx.admission.close();
		ctx.guard.release();
		assert.equal(
			env.worker.messages.some((m) => m.kind === 'open'),
			false,
		);
	}
});
test('Source7 frame holds block close/reuse and full BEGIN monotonic/fixed intent guards remain exact', async () => {
	const env = setup(),
		{ client, p, guard } = await opened(env),
		request = await begin(p, true);
	const frame = await client.update(request);
	await assert.rejects(client.close(), { code: 'ResourceLimit' });
	assert.equal(frame.pixels.length, frame.width * frame.height * 4);
	frame.release();
	frame.release();
	await assert.rejects(client.update(request), { code: 'ConflictStaleBase' });
	await assert.rejects(client.base({ ...request, operation: {} }), {
		code: 'InputSchema',
	});
	const target = '55555555-5555-5555-5555-555555555555';
	await assert.rejects(
		client.update({
			...request,
			generation: '2',
			target,
			operation: {
				op: 'filter.set_enabled',
				layer_ref: { id: target },
				filter_ref: { id: '66666666-6666-6666-6666-666666666666' },
				enabled: false,
			},
		}),
		{ code: 'ConflictStaleBase' },
	);
	await client.close();
	guard.release();
	assert.equal(env.broker.usage().reserved, 0);
});
test('Source7 latest wanted coalesces behind genuine cancel ACK and never publishes superseded output', async () => {
	const env = setup(),
		{ client, p, guard } = await opened(env),
		first = await begin(p);
	const original = env.worker.normal.bind(env.worker);
	let held, cancel;
	env.worker.handler = (m) => {
		if (m.kind === 'update' && !held) {
			held = m;
			return;
		}
		if (m.kind === 'cancel' && !cancel) {
			cancel = m;
			return;
		}
		return original(m);
	};
	const one = client.update(first),
		oneRejected = assert.rejects(one, { code: 'OperationCancelled' });
	await env.worker.wait('update');
	const two = client.update({ ...first, generation: '2' }),
		twoRejected = assert.rejects(two, { code: 'OperationCancelled' });
	await env.worker.wait('cancel');
	const three = client.update({ ...first, generation: '3' });
	env.worker.reply(held, { kind: 'retired', generation: '1' });
	assert.equal(
		env.worker.messages.filter((m) => m.kind === 'update').length,
		1,
	);
	env.worker.reply(cancel, { kind: 'retired', generation: '1' });
	const latest = await three;
	assert.equal(latest.generation, '3');
	latest.release();
	await oneRejected;
	await twoRejected;
	await client.close();
	guard.release();
});
test('Source7 uncertain Worker timeout quarantines its one lease rather than retry/reopen', async () => {
	const env = setup(5);
	env.worker.handler = () => {};
	const admission = env.controller.beginLayerAppearancePreview(preplan);
	await assert.rejects(admission.ready, { code: 'CodecFailed' });
	assert.equal(env.broker.usage().quarantined, true);
	assert.throws(() => env.controller.beginLayerAppearancePreview(preplan), {
		code: 'ResourceLimit',
	});
});
test('Source7 typed HELLO refusal precedes transfer and still joins close', async () => {
	const env = setup(),
		ctx = await prepared(env),
		original = env.worker.normal.bind(env.worker);
	env.worker.handler = (m) => {
		if (m.kind === 'hello')
			return env.worker.reply(m, {
				kind: 'error',
				completed: true,
				code: 'capability.unsupported',
				message: 'closed class',
			});
		return original(m);
	};
	await assert.rejects(
		ctx.admission.open(new Uint8Array([1]), ctx.p.sourceSha256, {
			plan: ctx.p,
			hello: ctx.hello,
		}),
		{ code: 'capability.unsupported' },
	);
	await ctx.admission.close();
	ctx.guard.release();
	assert.equal(
		env.worker.messages.some((m) => m.kind === 'open'),
		false,
	);
	assert.equal(env.broker.usage().reserved, 0);
});
test('Source7 malformed frame cannot escape as a held output or return unjoined credit', async () => {
	const env = setup(),
		{ client, p, guard } = await opened(env),
		original = env.worker.normal.bind(env.worker);
	env.worker.handler = async (m) => {
		if (m.kind === 'update')
			return env.worker.reply(m, {
				...(await binding(p, m.request)),
				pixels: new Uint8Array(1),
			});
		return original(m);
	};
	await assert.rejects(client.update(await begin(p, true)), {
		code: 'CodecFailed',
	});
	guard.release();
	assert.equal(env.broker.usage().quarantined, true);
});
test('Source7 close during HELLO cancels suppressed bootstrap ids only on genuine closed ACK', async () => {
	const env = setup(),
		ctx = await prepared(env),
		original = env.worker.normal.bind(env.worker);
	env.worker.handler = (m) => (m.kind === 'hello' ? undefined : original(m));
	const opening = ctx.admission.open(
		new Uint8Array([1]),
		ctx.p.sourceSha256,
		{ plan: ctx.p, hello: ctx.hello },
	);
	const rejected = assert.rejects(opening, { code: 'OperationCancelled' });
	await env.worker.wait('hello');
	await ctx.admission.close();
	await rejected;
	assert.notEqual(env.broker.usage().reserved, 0);
	ctx.guard.release();
	assert.equal(env.broker.usage().reserved, 0);
});
test('Source7 copy SHA finalization joins cancellation and close, rejecting stale publication', async () => {
	const env = setup(),
		{ client, p, guard } = await opened(env),
		original = env.worker.normal.bind(env.worker);
	const request = {
			...(await begin(p, true)),
			view: { region: [0, 0, 5, 3], level: 0 },
		},
		output = {
			version: 1,
			purpose: 'print',
			format: 'png',
			quality: 90,
			matte: [255, 255, 255],
			frontend: 'studio',
			omitMetadata: false,
			bakeFilters: false,
			maskLength: 0,
			maskSha256: await sha(new Uint8Array()),
		},
		outputSha256 = await sha(
			new TextEncoder().encode(encodeProgram(output, 4096)),
		),
		bytes = new Uint8Array([9, 8, 7, 6]),
		bytesSha256 = await sha(bytes),
		entered = deferred(),
		release = deferred(),
		previous = globalThis.crypto;
	let preparedBinding;
	env.worker.handler = async (m) => {
		if (m.kind === 'copy-prepare') {
			preparedBinding = await binding(p, m.request);
			return env.worker.reply(m, {
				kind: 'copy-admitted',
				outputVersion: 1,
				outputSha256,
				binding: preparedBinding,
			});
		}
		if (m.kind === 'copy-input')
			return env.worker.reply(m, {
				kind: 'encoded-copy',
				outputVersion: 1,
				outputSha256,
				bytesSha256,
				mime: 'image/png',
				binding: preparedBinding,
				bytes,
			});
		return original(m);
	};
	Object.defineProperty(globalThis, 'crypto', {
		configurable: true,
		value: {
			subtle: {
				async digest(algorithm, input) {
					if (
						input.byteLength === 4 &&
						new Uint8Array(
							input.buffer ?? input,
							input.byteOffset ?? 0,
							input.byteLength,
						)[0] === 9
					) {
						entered.resolve();
						await release.promise;
					}
					return webcrypto.subtle.digest(algorithm, input);
				},
			},
		},
	});
	try {
		const copying = client.encodedCopy(request, output),
			rejected = assert.rejects(copying, { code: 'OperationCancelled' });
		await entered.promise;
		const closing = client.close();
		await env.worker.wait('cancel');
		assert.equal(
			env.worker.messages.some((m) => m.kind === 'close'),
			false,
		);
		assert.notEqual(env.broker.usage().reserved, 0);
		release.resolve();
		await rejected;
		await closing;
		guard.release();
		assert.equal(env.broker.usage().reserved, 0);
	} finally {
		release.resolve();
		Object.defineProperty(globalThis, 'crypto', {
			configurable: true,
			value: previous,
		});
	}
});
test('Source7 repeated cancel calls share one pending cut barrier and one latest successor', async () => {
	const env = setup(),
		{ client, guard } = await opened(env),
		original = env.worker.normal.bind(env.worker);
	let pending;
	env.worker.handler = (m) => {
		if (m.kind === 'cancel' && !pending) {
			pending = m;
			return;
		}
		return original(m);
	};
	const first = client.cancel('1');
	await env.worker.wait('cancel');
	const again = Array.from({ length: 100 }, () => client.cancel('1'));
	const later = client.cancel('2');
	assert.equal(
		env.worker.messages.filter((m) => m.kind === 'cancel').length,
		1,
	);
	env.worker.reply(pending, { kind: 'retired', generation: '1' });
	await first;
	await Promise.all(again);
	await later;
	assert.equal(
		env.worker.messages.filter((m) => m.kind === 'cancel').length,
		2,
	);
	await client.close();
	guard.release();
});
test('Source7 Main credit release before delayed closed ACK does not release the Worker slot', async () => {
	const env = setup(),
		{ admission, guard } = await prepared(env),
		original = env.worker.normal.bind(env.worker);
	let pending;
	env.worker.handler = (m) => {
		if (m.kind === 'close') {
			pending = m;
			return;
		}
		return original(m);
	};
	const closing = admission.close();
	await env.worker.wait('close');
	guard.release();
	assert.notEqual(env.broker.usage().reserved, 0);
	assert.throws(() => env.controller.beginLayerAppearancePreview(preplan), {
		code: 'ResourceLimit',
	});
	env.worker.reply(pending, { kind: 'closed' });
	await closing;
	assert.equal(env.broker.usage().reserved, 0);
});

test('Source7 preparation wrapper rejects accessors before ownership or Worker adoption', async () => {
	for (const key of ['plan', 'hello']) {
		const env = setup();
		const context = await prepared(env);
		const wrapper = { plan: context.p, hello: context.hello };
		let reads = 0;
		Object.defineProperty(wrapper, key, {
			enumerable: true,
			get() {
				reads++;
				return key === 'plan' ? context.p : context.hello;
			},
		});
		const source = new Uint8Array([1, 2, 3]);
		await assert.rejects(
			context.admission.open(source, context.p.sourceSha256, wrapper),
			{ code: 'InputSchema' },
		);
		assert.equal(reads, 0);
		assert.equal(source.byteLength, 3);
		assert.equal(
			env.worker.messages.some((m) => ['hello', 'open'].includes(m.kind)),
			false,
		);
		await context.admission.close();
		context.guard.release();
		assert.deepEqual(env.broker.usage(), {
			reserved: 0,
			quarantined: false,
		});
	}
});
