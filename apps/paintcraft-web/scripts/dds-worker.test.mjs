import test from 'node:test';
import assert from 'node:assert/strict';
import {
	freezeDdsPlan,
	captureDds,
	decodeDdsResponse,
	encodeDdsRequest,
	isCompletedDdsRefusal,
	DDS_RESERVATION,
	DDS_RESPONSE_HEADER,
} from '../src/lib/dds-worker-protocol.js';
const owner = Object.freeze({
	session: '00000000-0000-4000-8000-000000000001',
	state: '00000000-0000-4000-8000-000000000002',
	epoch: 7n,
	frontend: 3,
});
const nonce = '00000000-0000-4000-8000-000000000003';
const version = '0.1.0';
function plan() {
	return {
		width: 1,
		height: 1,
		sourceFlags: 0,
		settings: new Uint8Array([0, 0, 0, 1, 0, 0, 0, 0]),
	};
}
async function fresh() {
	return import(`../src/lib/codec-worker.js?dds-test=${crypto.randomUUID()}`);
}
async function response(
	header,
	{
		status = 0,
		encoded = new Uint8Array([1, 2, 3]),
		preview = new Uint8Array([55, 128, 255, 127]),
		detail = '',
		count = 0,
	} = {},
) {
	const text = new TextEncoder(),
		msg = text.encode(detail),
		app = text.encode(version);
	if (status) {
		encoded = new Uint8Array();
		preview = new Uint8Array();
	}
	const bytes = new Uint8Array(
			DDS_RESPONSE_HEADER + encoded.length + preview.length + msg.length,
		),
		v = new DataView(bytes.buffer);
	bytes.set(text.encode('PCDDSR01'));
	v.setUint16(8, 1);
	bytes[10] = status;
	bytes.set(header.subarray(16, 24), 12);
	bytes.set(header.subarray(12, 16), 20);
	v.setUint32(24, encoded.length);
	v.setUint16(28, status ? 0 : 1);
	v.setUint16(30, status ? 0 : 1);
	bytes.set(header.subarray(24, 65), 32);
	bytes.set(header.subarray(65, 81), 73);
	bytes.set(header.subarray(81, 113), 89);
	bytes.set(header.subarray(113, 121), 121);
	bytes.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', encoded)),
		129,
	);
	bytes.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', preview)),
		161,
	);
	bytes[193] = status
		? 0
		: header[121] | (header[113] ? 4 : 0) | (count ? 16 : 0);
	v.setUint32(194, count);
	bytes[198] = app.length;
	bytes.set(app, 199);
	v.setUint16(263, msg.length);
	bytes[265] = header[121];
	bytes.set(encoded, DDS_RESPONSE_HEADER);
	bytes.set(preview, DDS_RESPONSE_HEADER + encoded.length);
	bytes.set(msg, DDS_RESPONSE_HEADER + encoded.length + preview.length);
	return bytes;
}
function options(createWorker, extra = {}) {
	return {
		owner,
		plan: plan(),
		nonce,
		currentOwner: () => owner,
		applicationVersion: version,
		createWorker,
		...extra,
	};
}
function worker(make) {
	return {
		stopped: false,
		terminate() {
			this.stopped = true;
		},
		postMessage(data) {
			assert.equal(data.kind, 'dds-export');
			Promise.resolve(make(new Uint8Array(data.bytes))).then((bytes) =>
				this.onmessage({
					data: { kind: 'result', bytes: bytes.buffer },
				}),
			);
		},
	};
}
test('public capture freezes source, owner, plan and options before digest awaits', async () => {
	const p = plan(),
		bytes = new Uint8Array([1, 2, 3]);
	let finish;
	const delayed = {
		subtle: {
			digest: () =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		},
	};
	const pending = captureDds(bytes, owner, p, nonce, delayed);
	p.width = 99;
	p.settings[0] = 3;
	bytes[0] = 9;
	finish(new Uint8Array(32).buffer);
	const captured = await pending;
	assert.equal(captured.plan.width, 1);
	assert.equal(captured.plan.settings[0], 0);
	assert.deepEqual([...captured.source], [1, 2, 3]);
	assert.equal(new DataView(captured.header.buffer).getUint32(16), 1);
	assert.throws(() => {
		captured.plan.settings[0] = 3;
	}, TypeError);
	assert.equal(encodeDdsRequest(captured).length, 131);
	for (const mutate of [
		(p) => {
			p.width = 1048577;
		},
		(p) => {
			p.settings[5] = 1;
		},
		(p) => {
			p.settings[4] = 1;
		},
		(p) => {
			p.settings[6] = 1;
		},
		(p) => {
			p.settings[3] = 2;
		},
	]) {
		const p = plan();
		mutate(p);
		assert.throws(() => freezeDdsPlan(p), { code: 'InputSchema' });
	}
});
test('two-phase admission precedes serialization and returns only a Rust-validation receipt', async () => {
	const m = await fresh();
	const ticket = m.admitDdsInWorker(
		options(() => worker((header) => response(header))),
	);
	assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	assert.throws(() => m.admitDdsInWorker(options()), {
		code: 'ResourceLimit',
	});
	const raw = await ticket.run(new Uint8Array([1, 2, 3]));
	assert.equal(raw.requiresRustValidation, true);
	assert.equal(raw.encoded, undefined);
	assert.equal(raw.preview, undefined);
	assert.equal(raw.requestHeader.length, 128);
	assert.equal(raw.sourceSha256.length, 32);
	assert.ok(raw.responseBytes.length > DDS_RESPONSE_HEADER);
	assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	// The synthetic payload above is not a DDS header. It must go to Rust's
	// decode_response before adoption; this intentionally unvalidated host path
	// quarantines rather than pretending that matching digests prove a DDS file.
	raw.quarantine();
	raw.release();
	assert.equal(m.codecWorkerBudget().quarantined, true);
	assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	await assert.rejects(ticket.run(new Uint8Array([1])), {
		code: 'InputSchema',
	});
});
test('unused admission retains charge on abort until caller explicitly drops its snapshots', async () => {
	const m = await fresh(),
		abort = new AbortController();
	const ticket = m.admitDdsInWorker(
		options(undefined, { signal: abort.signal }),
	);
	abort.abort();
	assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	ticket.release();
	assert.equal(m.codecWorkerBudget().reservedBytes, 0);
	assert.equal(m.codecWorkerBudget().quarantined, false);
	await assert.rejects(ticket.run(new Uint8Array([1])), {
		code: 'InputSchema',
	});
});
test('known completed refusal permits retry; forged, replayed and malformed responses do not', async () => {
	const captured = await captureDds(
		new Uint8Array([1]),
		owner,
		plan(),
		nonce,
	);
	const valid = await response(captured.header, {
		status: 3,
		detail: 'bounded refusal',
	});
	let completed;
	try {
		await decodeDdsResponse(valid, captured, owner, version);
	} catch (error) {
		completed = error;
	}
	assert.equal(isCompletedDdsRefusal(completed, captured), true);
	assert.equal(
		isCompletedDdsRefusal(
			Object.assign(new Error(), { codecCompleted: true }),
			captured,
		),
		false,
	);
	const other = await captureDds(new Uint8Array([1]), owner, plan(), nonce);
	assert.equal(isCompletedDdsRefusal(completed, other), false);
	const m = await fresh();
	for (let i = 0; i < 2; i++) {
		const ticket = m.admitDdsInWorker(
			options(() =>
				worker((header) =>
					response(header, { status: 3, detail: 'bounded refusal' }),
				),
			),
		);
		await assert.rejects(ticket.run(new Uint8Array([1])), {
			code: 'ResourceLimit',
		});
		assert.equal(m.codecWorkerBudget().reservedBytes, 0);
		assert.equal(m.codecWorkerBudget().quarantined, false);
	}
	for (const at of [
		0, 8, 11, 12, 20, 24, 28, 32, 73, 89, 121, 129, 161, 193, 194, 198, 199,
		263, 265, 266, 287,
	]) {
		const bad = (await response(captured.header)).slice();
		bad[at] ^= 255;
		await assert.rejects(decodeDdsResponse(bad, captured, owner, version), {
			code: 'CodecFailed',
		});
	}
	await assert.rejects(
		decodeDdsResponse(
			await response(captured.header),
			captured,
			{ ...owner, epoch: 8n },
			version,
		),
		{ code: 'ConflictStaleBase' },
	);
});
test('running cancellation returns promptly, retains the declaration and blocks later imports/exports', async () => {
	const m = await fresh(),
		abort = new AbortController();
	const ticket = m.admitDdsInWorker(
		options(
			() => ({
				terminate() {},
				postMessage() {
					abort.abort(
						Object.assign(new Error('forged'), {
							codecCompleted: true,
						}),
					);
				},
			}),
			{ signal: abort.signal },
		),
	);
	await assert.rejects(ticket.run(new Uint8Array([1])), {
		code: 'OperationCancelled',
	});
	assert.equal(m.codecWorkerBudget().quarantined, true);
	assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	assert.throws(() => m.admitDdsInWorker(options()), {
		code: 'ResourceLimit',
	});
	await assert.rejects(
		m.decodeInWorker({
			source: new Uint8Array([1]),
			owner,
			codec: 2,
			pixels: 1,
			currentOwner: () => owner,
			applicationVersion: version,
		}),
		{ code: 'ResourceLimit' },
	);
});
test('malformed worker output and postMessage/worker failures cannot acknowledge completion', async () => {
	for (const createWorker of [
		() => {
			throw Object.assign(new Error('startup'), { codecCompleted: true });
		},
		() => ({
			terminate() {},
			postMessage() {
				throw Object.assign(new Error('send'), {
					codecCompleted: true,
				});
			},
		}),
		() =>
			worker(async (header) => {
				const bytes = await response(header);
				bytes[129] ^= 1;
				return bytes;
			}),
	]) {
		const m = await fresh(),
			ticket = m.admitDdsInWorker(options(createWorker));
		await assert.rejects(ticket.run(new Uint8Array([1])));
		assert.equal(m.codecWorkerBudget().quarantined, true);
		assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	}
});

test('owner change or an invalid source before Worker startup releases known unused admission', async () => {
	for (const invalidSource of [false, true]) {
		const m = await fresh();
		let current = owner,
			launches = 0;
		const ticket = m.admitDdsInWorker(
			options(
				() => {
					launches++;
					throw new Error('must not launch');
				},
				{ currentOwner: () => current },
			),
		);
		if (!invalidSource) current = { ...owner, epoch: 99n };
		await assert.rejects(
			ticket.run(invalidSource ? new Uint8Array() : new Uint8Array([1])),
			{ code: invalidSource ? 'ResourceLimit' : 'ConflictStaleBase' },
		);
		assert.equal(launches, 0);
		assert.equal(m.codecWorkerBudget().reservedBytes, 0);
		assert.equal(m.codecWorkerBudget().quarantined, false);
	}
});
test('bridge admission cannot rebind a stale owner and keeps the service exclusive before serialization', async () => {
	const { createCodecService } = await import('../src/lib/codec-bridge.js');
	const service = createCodecService(version);
	service.setOwner(owner);
	assert.equal(service.ddsExportProtocolVersion(), 1);
	assert.throws(
		() => service.beginDdsExport({ ...owner, epoch: 9n }, plan(), nonce),
		{ code: 'ConflictStaleBase' },
	);
	const ticket = service.beginDdsExport(owner, plan(), nonce);
	assert.equal(service.budget().reservedBytes, DDS_RESERVATION);
	assert.throws(() => service.beginDdsExport(owner, plan(), nonce), {
		code: 'ResourceLimit',
	});
	service.cancel();
	assert.equal(service.budget().reservedBytes, DDS_RESERVATION);
	ticket.release();
	assert.equal(service.budget().reservedBytes, 0);
	const next = service.beginDdsExport(owner, plan(), nonce);
	next.release();
});

test('DDS ignores duplicate and late messages before touching their payloads or starting new digests', async () => {
	const m = await fresh();
	let instance,
		reads = 0,
		calls = 0;
	const original = crypto.subtle.digest.bind(crypto.subtle);
	const ticket = m.admitDdsInWorker(
		options(() => ({
			terminate() {},
			async postMessage(data) {
				instance = this;
				const bytes = await response(new Uint8Array(data.bytes), {
					status: 3,
					detail: 'complete refusal',
				});
				// The first validation starts asynchronously. A duplicate's data getter
				// must remain untouched, including while the output digests are pending.
				this.onmessage({
					data: { kind: 'result', bytes: bytes.buffer },
				});
				this.onmessage({
					get data() {
						reads++;
						throw new Error('duplicate payload');
					},
				});
			},
		})),
	);
	crypto.subtle.digest = (...args) => {
		calls++;
		return original(...args);
	};
	try {
		await assert.rejects(ticket.run(new Uint8Array([1])), {
			code: 'ResourceLimit',
		});
	} finally {
		crypto.subtle.digest = original;
	}
	assert.equal(calls, 5); // request SHA + response construction two + validation two
	assert.equal(reads, 0);
	instance.onmessage({
		get data() {
			reads++;
			throw new Error('late payload');
		},
	});
	instance.onerror();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.equal(reads, 0);
	assert.equal(m.codecWorkerBudget().reservedBytes, 0);
	assert.equal(m.codecWorkerBudget().quarantined, false);
});

test('a hung unjoinable source digest cancels promptly but never releases its reservation', async () => {
	const m = await fresh(),
		abort = new AbortController();
	let finish,
		launches = 0;
	const original = crypto.subtle.digest;
	crypto.subtle.digest = () =>
		new Promise((resolve) => {
			finish = resolve;
		});
	try {
		const ticket = m.admitDdsInWorker(
			options(
				() => {
					launches++;
					throw new Error('must not launch');
				},
				{ signal: abort.signal },
			),
		);
		const pending = ticket.run(new Uint8Array([1]));
		abort.abort();
		await assert.rejects(pending, { code: 'OperationCancelled' });
		assert.equal(launches, 0);
		assert.equal(m.codecWorkerBudget().quarantined, true);
		assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
		finish(new Uint8Array(32).buffer);
		await new Promise((resolve) => setTimeout(resolve, 0));
		assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	} finally {
		crypto.subtle.digest = original;
	}
});
test('a running job deadline or changed owner quarantines and ignores subsequent late success', async () => {
	for (const stale of [false, true]) {
		const m = await fresh();
		let current = owner,
			instance,
			sent;
		const ticket = m.admitDdsInWorker(
			options(
				() => ({
					terminate() {},
					postMessage(data) {
						instance = this;
						sent = new Uint8Array(data.bytes).slice();
						if (stale) current = { ...owner, epoch: 8n };
					},
				}),
				{ currentOwner: () => current, deadlineMs: 30 },
			),
		);
		await assert.rejects(ticket.run(new Uint8Array([1])), {
			code: stale ? 'ConflictStaleBase' : 'ResourceLimit',
		});
		assert.equal(m.codecWorkerBudget().quarantined, true);
		const late = await response(sent);
		instance.onmessage({ data: { kind: 'result', bytes: late.buffer } });
		await new Promise((resolve) => setTimeout(resolve, 0));
		assert.equal(m.codecWorkerBudget().reservedBytes, DDS_RESERVATION);
	}
});
