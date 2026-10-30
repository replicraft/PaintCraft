// Actual service/controller lifetime tests with only the Worker substituted.
import assert from 'node:assert/strict';
import test from 'node:test';
import { createCodecService } from '../src/lib/codec-bridge.js';
import {
	ownerBytes,
	RESPONSE_HEADER,
} from '../src/lib/codec-worker-protocol.js';
import { DDS_RESPONSE_HEADER } from '../src/lib/dds-worker-protocol.js';
import { context } from './test-support/source-metadata-context.mjs';

const owner = Object.freeze({
	session: '00000000-0000-4000-8000-000000000001',
	state: '00000000-0000-4000-8000-000000000002',
	epoch: 7n,
	frontend: 3,
});
const version = '0.1.0';
const text = new TextEncoder();
const mono = () => ({
	width: 1,
	height: 1,
	sourceFlags: 0,
	filterDigest: new Uint8Array(32).fill(17),
	bakeIntent: 0,
	version: 2,
	maskContext: new Uint8Array(32),
	metadataContext: context(2),
});
const dds = () => ({
	width: 1,
	height: 1,
	sourceFlags: 0,
	filterDigest: new Uint8Array(32).fill(17),
	bakeIntent: 0,
	maskContext: new Uint8Array(32),
	metadataContext: context(1),
	settings: new Uint8Array([0, 0, 0, 1, 0, 0, 0, 0]),
});
const begin = (service, kind) =>
	kind === 'msp-export'
		? service.beginMspExport(owner, mono())
		: service.beginDdsExport(owner, dds());
async function refusal(kind, request) {
	const app = text.encode(version),
		detail = text.encode('known completed refusal');
	const header = kind === 'decode' ? RESPONSE_HEADER : DDS_RESPONSE_HEADER;
	const bytes = new Uint8Array(
		header + (kind === 'decode' ? app.length : 0) + detail.length,
	);
	const v = new DataView(bytes.buffer);
	bytes[10] = 3;
	if (kind === 'decode') {
		bytes.set(text.encode('PCPIX002'));
		v.setUint16(8, 2);
		bytes[11] = request[10];
		v.setUint16(36, detail.length);
		bytes[38] = app.length;
		bytes.set(request.subarray(23, 64), 47);
		bytes.set(request.subarray(64, 80), 88);
		bytes.set(request.subarray(80, 112), 104);
		bytes.set(app, header);
		bytes.set(detail, header + app.length);
	} else {
		bytes.set(text.encode(kind === 'dds-export' ? 'PCDDSR04' : 'PCMSPR04'));
		v.setUint16(8, 4);
		bytes.set(request.subarray(16, 24), 12);
		bytes.set(request.subarray(12, 16), 20);
		bytes.set(request.subarray(24, 65), 32);
		bytes.set(request.subarray(65, 81), 73);
		bytes.set(request.subarray(81, 113), 89);
		if (kind === 'dds-export') bytes.set(request.subarray(113, 121), 121);
		else bytes[121] = 2;
		const empty = new Uint8Array(
			await crypto.subtle.digest('SHA-256', new Uint8Array()),
		);
		bytes.set(empty, 129);
		bytes.set(empty, 161);
		bytes[198] = app.length;
		bytes.set(app, 199);
		v.setUint16(263, detail.length);
		bytes[265] = request[kind === 'dds-export' ? 121 : 114];
		bytes[266] = request[122];
		bytes.set(request.subarray(128, 160), 288);
		bytes.set(request.subarray(160, 224), 320);
		bytes.set(request.subarray(224, 256), 384);
		bytes.set(detail, header);
	}
	return bytes;
}
function installWorker() {
	const original = globalThis.Worker,
		instances = [];
	globalThis.Worker = class {
		constructor() {
			instances.push(this);
			this.stopped = false;
		}
		terminate() {
			this.stopped = true;
		}
		postMessage(data) {
			this.kind = data.kind;
			this.header = new Uint8Array(data.bytes).slice();
		}
		async refuse() {
			const bytes = await refusal(this.kind, this.header);
			await this.onmessage({
				data: { kind: 'result', bytes: bytes.buffer },
			});
		}
	};
	return {
		instances,
		restore() {
			globalThis.Worker = original;
		},
	};
}
async function posted(instances, count) {
	const until = performance.now() + 1000;
	while (instances.length < count || !instances[count - 1].header) {
		assert.ok(performance.now() < until, 'Worker request did not start');
		await new Promise((resolve) => setTimeout(resolve, 0));
	}
	return instances[count - 1];
}
const outcome = (promise) =>
	promise.then(
		(value) => ({ value }),
		(error) => ({ error }),
	);

const file = () => new File([new Uint8Array([1])], 'owned.jxl');
test('an old completed import ticket cannot cancel a newer import or export', async () => {
	const workers = installWorker();
	try {
		for (const kind of ['decode', 'dds-export', 'msp-export']) {
			const service = createCodecService(version);
			service.setOwner(owner);
			const count = workers.instances.length;
			const old = service.beginDecodeFile(file(), owner, 1);
			const first = outcome(old.run());
			await (await posted(workers.instances, count + 1)).refuse();
			assert.equal((await first).error.code, 'ResourceLimit');
			assert.equal(service.budget().reservedBytes, 0);
			const next =
				kind === 'decode'
					? service.beginDecodeFile(file(), owner, 1)
					: begin(service, kind);
			const running = outcome(next.run(new Uint8Array([1])));
			const current = await posted(workers.instances, count + 2);
			const charged = service.budget().reservedBytes;
			old.cancel();
			old.release();
			await new Promise((resolve) => setTimeout(resolve, 20));
			assert.equal(
				current.stopped,
				false,
				`old import cancelled newer ${kind}`,
			);
			assert.equal(service.budget().reservedBytes, charged);
			assert.equal(service.budget().quarantined, false);
			await current.refuse();
			assert.equal((await running).error.code, 'ResourceLimit');
			assert.equal(service.budget().reservedBytes, 0);
			assert.equal(service.budget().quarantined, false);
		}
	} finally {
		workers.restore();
	}
});
test('import admission freezes its owner and unused tickets do not read or reserve a file', async () => {
	const workers = installWorker();
	try {
		const service = createCodecService(version);
		service.setOwner(owner);
		const stopped = service.beginDecodeFile(file(), owner, 1);
		stopped.cancel();
		await assert.rejects(stopped.run(), { code: 'OperationCancelled' });
		assert.equal(workers.instances.length, 0);
		assert.equal(service.budget().reservedBytes, 0);
		assert.equal(service.budget().quarantined, false);
		const unused = service.beginDecodeFile(file(), owner, 1);
		assert.equal(service.budget().reservedBytes, 0);
		unused.cancel();
		unused.release();
		await assert.rejects(unused.run(), { code: 'InputSchema' });
		assert.equal(workers.instances.length, 0);
		assert.equal(service.budget().quarantined, false);
		const mutable = { ...owner };
		const ticket = service.beginDecodeFile(file(), mutable, 1);
		mutable.epoch = 99n;
		const running = outcome(ticket.run());
		const current = await posted(workers.instances, 1);
		assert.deepEqual(current.header.slice(23, 64), ownerBytes(owner));
		await current.refuse();
		assert.equal((await running).error.code, 'ResourceLimit');
		assert.equal(service.budget().reservedBytes, 0);
		assert.equal(service.budget().quarantined, false);
	} finally {
		workers.restore();
	}
});
test('the import ticket still cancels its own active job without releasing unknown lifetime', async () => {
	const workers = installWorker();
	try {
		const service = createCodecService(version);
		service.setOwner(owner);
		const ticket = service.beginDecodeFile(file(), owner, 1);
		const running = outcome(ticket.run());
		const current = await posted(workers.instances, 1);
		const charged = service.budget().reservedBytes;
		ticket.cancel();
		assert.equal((await running).error.code, 'OperationCancelled');
		assert.equal(current.stopped, true);
		assert.ok(charged > 0);
		assert.equal(service.budget().reservedBytes, charged);
		assert.equal(service.budget().quarantined, true);
		ticket.release();
		assert.equal(service.budget().reservedBytes, charged);
		await assert.rejects(service.decodeFile(file(), owner, 1), {
			code: 'ResourceLimit',
		});
	} finally {
		workers.restore();
	}
});
