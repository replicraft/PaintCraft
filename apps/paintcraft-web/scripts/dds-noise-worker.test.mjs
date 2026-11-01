import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash, webcrypto } from 'node:crypto';
import test from 'node:test';
import {
	freezeNoiseContexts,
	verifyNoiseWorkload,
} from '../src/lib/source-noise-export-context.js';
import {
	captureDds,
	encodeDdsRequest,
} from '../src/lib/dds-noise-worker-protocol.js';
import {
	admitDdsInWorker,
	admitMspInWorker,
	codecWorkerBudget,
} from '../src/lib/codec-worker.js';
const vector = JSON.parse(
	readFileSync(
		new URL('./fixtures/dds-noise-wire/vector.json', import.meta.url),
	),
);
const bytes = (s) => new Uint8Array(Buffer.from(s, 'hex'));
const owner = {
	session: '00000000-0000-0000-0000-000000000001',
	state: '00000000-0000-0000-0000-000000000002',
	epoch: 1n,
	frontend: 3,
};
function plan() {
	return {
		protocolVersion: 8,
		width: 1,
		height: 1,
		sourceFlags: 4,
		bakeIntent: 1,
		filterDigest: new Uint8Array(32),
		settings: new Uint8Array([0, 0, 0, 1, 0, 0, 0, 0]),
		metadataContext: new Uint8Array(64),
		maskContext: new Uint8Array(32),
		noiseWorkload: bytes(vector.workload),
		noiseRegionContext: bytes(vector.region),
		noiseProgramContext: bytes(vector.program),
	};
}
// Existing metadata grammar requires target DDS1 even when no retained metadata.
function validPlan() {
	const p = plan();
	p.metadataContext[49] = 1;
	p.metadataContext[50] = 1;
	return p;
}
test('independent descriptive context vector bounds and workload digest', async () => {
	const p = validPlan();
	await verifyNoiseWorkload(p.noiseWorkload, webcrypto);
	assert.equal(
		freezeNoiseContexts(
			p.noiseRegionContext,
			p.noiseProgramContext,
			p.noiseWorkload,
			192 << 20,
		).program.length,
		256,
	);
	for (const at of [0, 2, 4, 6, 112]) {
		const r = p.noiseRegionContext.slice();
		r[at] ^= 1;
		assert.throws(() =>
			freezeNoiseContexts(
				r,
				p.noiseProgramContext,
				p.noiseWorkload,
				192 << 20,
			),
		);
	}
	const bad = p.noiseWorkload.slice();
	bad[32] ^= 1;
	await assert.rejects(verifyNoiseWorkload(bad, webcrypto));
});
test('DDS8 literal header offsets detach all inputs before asynchronous hash', async () => {
	const p = validPlan(),
		source = new Uint8Array(128);
	const expectedRegion = bytes(vector.region),
		expectedProgram = bytes(vector.program),
		expectedWorkload = bytes(vector.workload);
	const pending = captureDds(
		source,
		owner,
		p,
		'00000000-0000-0000-0000-000000000003',
		webcrypto,
	);
	// This is before either async workload/source digest resolves. The request
	// must still contain the independent original literals and original source.
	source.fill(0xaa);
	p.noiseRegionContext.fill(0x44);
	p.noiseProgramContext.fill(0x55);
	p.noiseWorkload.fill(0x66);
	structuredClone(source.buffer, { transfer: [source.buffer] });
	assert.equal(
		source.length,
		0,
		'original source can detach after ownership capture',
	);
	const c = await pending,
		wire = encodeDdsRequest(c);
	assert.equal(wire.length, 704 + 128);
	assert.equal(Buffer.from(wire.subarray(0, 8)).toString(), 'PCDDSQ08');
	assert.deepEqual(wire.subarray(256, 384), expectedRegion);
	assert.deepEqual(wire.subarray(384, 640), expectedProgram);
	assert.deepEqual(wire.subarray(640, 704), expectedWorkload);
	assert.deepEqual(wire.subarray(704), new Uint8Array(128));
	assert.deepEqual(
		wire.subarray(81, 113),
		new Uint8Array(
			createHash('sha256').update(new Uint8Array(128)).digest(),
		),
	);
	await assert.rejects(
		captureDds(
			new Uint8Array(127),
			owner,
			validPlan(),
			undefined,
			webcrypto,
		),
		/DDS8 actual source length/,
	);
});
test('futureMSP9 and malformedDDS8-as-MSP8 refuse before Worker; immutableDDSworkload staysbound', async () => {
	const p = validPlan();
	const options = {
		plan: p,
		owner,
		nonce: '00000000-0000-0000-0000-000000000003',
		applicationVersion: '0.1.0',
		currentOwner: () => owner,
	};
	let mspWorkers = 0;
	const mspOptions = {
		...options,
		createWorker: () => {
			mspWorkers++;
			throw Error('must not construct');
		},
	};
	assert.throws(
		() =>
			admitMspInWorker({
				...mspOptions,
				plan: { ...p, protocolVersion: 9 },
			}),
		/Unsupported export protocol/,
	);
	assert.throws(
		() => admitMspInWorker(mspOptions),
		(e) => e.code === 'InputSchema',
	);
	assert.equal(mspWorkers, 0);
	assert.equal(codecWorkerBudget().reservedBytes, 0);
	const admission = admitDdsInWorker(options);
	const changed = validPlan();
	changed.noiseWorkload = changed.noiseWorkload.slice();
	new DataView(changed.noiseWorkload.buffer).setBigUint64(16, 1n);
	try {
		await assert.rejects(admission.run(new Uint8Array(128), changed));
	} finally {
		admission.release();
	}
	assert.equal(codecWorkerBudget().reservedBytes, 0);
});
