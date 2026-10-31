import assert from 'node:assert/strict';
import { createHash, webcrypto } from 'node:crypto';
import test from 'node:test';
import {
	CAPABILITY_BYTES_MAX,
	encodeProgram,
	requirementDigests,
	unionPairs,
	validateCapabilities,
	validatePairs,
} from '../src/lib/filter-preview-program-protocol.js';

globalThis.crypto ??= webcrypto;
const pair = { profile_id: 'exposure-srgb-v1', codec_id: 'params-v1' };
const entry = {
	...pair,
	resource_contract_id: 'prepared-exposure-rgba8-v1',
	resource_contract_revision: 1,
};
function independent(pairs, entries) {
	const bytes = (domain, resolved) => {
		const chunks = [Buffer.from(domain)];
		const u16 = (n) => {
			const b = Buffer.alloc(2);
			b.writeUInt16BE(n);
			chunks.push(b);
		};
		const text = (s) => {
			const b = Buffer.from(s);
			u16(b.length);
			chunks.push(b);
		};
		if (resolved) u16(1);
		u16(pairs.length);
		pairs.forEach((p, i) => {
			text(p.profile_id);
			text(p.codec_id);
			if (resolved) {
				text(entries[i].resource_contract_id);
				const b = Buffer.alloc(4);
				b.writeUInt32BE(entries[i].resource_contract_revision);
				chunks.push(b);
			}
		});
		return createHash('sha256').update(Buffer.concat(chunks)).digest('hex');
	};
	return {
		count: pairs.length,
		requiredSha256: bytes('paintcraft.worker-required/v1\0', false),
		resolutionSha256: bytes('paintcraft.worker-resolution/v1\0', true),
	};
}
test('prepared control encoder refuses during traversal before later payload access', () => {
	let touched = false;
	const hostile = {
		a: 'x'.repeat(CAPABILITY_BYTES_MAX),
		get z() {
			touched = true;
			throw Error('must not visit');
		},
	};
	assert.throws(
		() => encodeProgram(hostile, CAPABILITY_BYTES_MAX),
		(e) => e.code === 'ResourceLimit',
	);
	assert.equal(touched, false);
	assert.equal(
		encodeProgram({ z: 'é\ud800', a: [null, true, 4] }, 100),
		'{' + '"a":[null,true,4],"z":"\\u00e9\\ud800"}',
	);
	const repeated = Array.from({ length: 257 }, () => ({
		a: 'x'.repeat(2048),
	}));
	assert.throws(
		() => encodeProgram(repeated, 65536),
		(e) => e.code === 'ResourceLimit',
	);
	assert.equal(encodeProgram('a', 3), '"a"');
	assert.throws(
		() => encodeProgram('a', 2),
		(e) => e.code === 'ResourceLimit',
	);
});
test('prepared capability subset hashes match independent binary framing', async () => {
	const actual = await requirementDigests([pair], [entry]);
	assert.deepEqual(actual, independent([pair], [entry]));
	const reply = {
		kind: 'capabilities',
		id: 7,
		protocol: 5,
		registryContract: 1,
		entries: [entry],
		requiredSha256: actual.requiredSha256,
		resolutionSha256: actual.resolutionSha256,
	};
	assert.deepEqual(await validateCapabilities(reply, [pair], actual), [
		entry,
	]);
	assert.deepEqual(await requirementDigests([], []), independent([], []));
	await assert.rejects(
		validateCapabilities(
			{
				...reply,
				entries: [{ ...entry, resource_contract_revision: 2 }],
			},
			[pair],
			actual,
		),
	);
	await assert.rejects(
		validateCapabilities(
			{ ...reply, entries: [entry, entry] },
			[pair],
			actual,
		),
	);
});
test('oversized capability identifiers are refused before encoding or digesting payloads', async () => {
	const expected = independent([pair], [entry]);
	const reply = {
		kind: 'capabilities',
		id: 1,
		protocol: 5,
		registryContract: 1,
		entries: [{ ...entry, resource_contract_id: 'x'.repeat(128 << 10) }],
		requiredSha256: expected.requiredSha256,
		resolutionSha256: expected.resolutionSha256,
	};
	await assert.rejects(
		validateCapabilities(reply, [pair], expected),
		(e) => e.code === 'InputSchema',
	);
});
test('immutable source union retains replaced algorithms and exact tuple ordering', () => {
	const base = [{ profile_id: 'invert-rgb-v1', codec_id: 'params-v1' }];
	assert.deepEqual(
		unionPairs(base, {
			op: 'filter.update',
			descriptor: { kind: 'exposure-srgb-v1' },
		}),
		[pair, ...base],
	);
	assert.deepEqual(unionPairs(base, { op: 'filter.remove' }), base);
	assert.equal(base.length, 1);
	for (const pairs of [
		[pair, pair],
		[base[0], pair],
		[{ ...pair, codec_id: 'params-v01' }],
	])
		assert.throws(() => validatePairs(pairs));
});

const uuid = '00000000-0000-4000-8000-000000000001';
async function mockClient({ refusal = false, tamper = false } = {}) {
	const messages = [];
	const source = Uint8Array.of(7, 8, 9),
		sourceSha256 = createHash('sha256').update(source).digest('hex');
	const empty = independent([], []);
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
		protocol: 5,
		registryContract: 1,
		workspaceBytes: 64 << 20,
		workLimit: 268435456,
		sourceRequirements: { ...empty, pairs: [] },
	};
	class Worker {
		postMessage(message, transfer = []) {
			messages.push({ message, transfer });
			queueMicrotask(async () => {
				const emit = (data) =>
					this.onmessage({ data: { ...data, id: message.id } });
				if (message.kind === 'hello') {
					if (refusal) {
						emit({
							kind: 'error',
							completed: true,
							code: 'capability.unsupported',
							message: 'unpriced',
						});
						return;
					}
					const entries = message.pairs.map((p) => ({
						...p,
						resource_contract_id:
							p.profile_id === 'exposure-srgb-v1'
								? 'prepared-exposure-rgba8-v1'
								: 'direct-rgba8-point-v1',
						resource_contract_revision: 1,
					}));
					const digests = independent(message.pairs, entries);
					emit({
						kind: 'capabilities',
						protocol: 5,
						registryContract: 1,
						entries,
						...(tamper
							? {
									requiredSha256: '0'.repeat(64),
									resolutionSha256: digests.resolutionSha256,
								}
							: {
									requiredSha256: digests.requiredSha256,
									resolutionSha256: digests.resolutionSha256,
								}),
					});
					return;
				}
				if (message.kind === 'open') {
					this.plan = message.plan;
					emit({
						kind: 'ready',
						binding: {
							...this.plan,
							candidateRequirements: empty,
							unionRequirements: empty,
						},
					});
					return;
				}
				if (message.kind === 'cancel') {
					emit({ kind: 'retired', generation: message.generation });
					return;
				}
				if (message.kind === 'close') {
					emit({ kind: 'closed' });
					return;
				}
				if (message.kind === 'update') {
					const r = message.request;
					const { canonical } = await import(
						'../src/lib/filter-preview-protocol.js'
					);
					emit({
						kind: r.render ? 'frame' : 'prepared',
						generation: r.generation,
						owner: this.plan.owner,
						draft: this.plan.draft,
						sourceSha256: this.plan.sourceSha256,
						target: r.target,
						view: r.view,
						width: 3,
						height: 2,
						region: [0, 0, 3, 2],
						level: 0,
						operationSha256: createHash('sha256')
							.update(canonical(r.operation))
							.digest('hex'),
						sourceRequirements: empty,
						candidateRequirements: r.candidateRequirements,
						unionRequirements: r.unionRequirements,
						...(r.render ? { pixels: new Uint8Array(24) } : {}),
					});
				}
			});
		}
		terminate() {
			this.terminated = true;
		}
	}
	globalThis.Worker = Worker;
	const service = await import(
		`../src/lib/filter-preview-worker.js?program=${Date.now()}-${Math.random()}`
	);
	return {
		messages,
		empty,
		service,
		plan,
		open: () => service.beginFilterPreview(plan).open(source, sourceSha256),
	};
}
test('Source5 HELLO precedes source transfer and resolves only each required subset', async () => {
	const h = await mockClient();
	const client = await h.open();
	assert.deepEqual(
		h.messages.map((x) => x.message.kind),
		['hello', 'open'],
	);
	assert.equal(h.messages[0].transfer.length, 0);
	assert.equal(h.messages[1].transfer.length, 1);
	assert.deepEqual(h.messages[0].message.pairs, []);
	let gen = 0;
	for (const p of [
		pair,
		{ profile_id: 'invert-rgb-v1', codec_id: 'params-v1' },
	]) {
		const e = {
			...p,
			resource_contract_id:
				p.profile_id === pair.profile_id
					? entry.resource_contract_id
					: 'direct-rgba8-point-v1',
			resource_contract_revision: 1,
		};
		const facts = independent([p], [e]);
		const r = await client.update({
			generation: String(++gen),
			operation: { op: 'filter.add', descriptor: { kind: p.profile_id } },
			target: uuid,
			view: { region: [0, 0, 3, 2], level: 0 },
			render: false,
			candidateRequirements: facts,
			unionRequirements: facts,
		});
		assert.equal(r.kind, 'prepared');
		r.release();
	}
	const base = await client.base({
		generation: '3',
		view: { region: [0, 0, 3, 2], level: 0 },
		candidateRequirements: h.empty,
		unionRequirements: h.empty,
	});
	base.release();
	assert.deepEqual(
		h.messages
			.filter((x) => x.message.kind === 'hello')
			.map((x) => x.message.pairs),
		[[], [pair], [{ profile_id: 'invert-rgb-v1', codec_id: 'params-v1' }]],
	);
	assert.equal(h.messages.filter((x) => x.message.kind === 'open').length, 1);
	await client.close();
	assert.equal(h.service.filterPreviewUsage().reserved, 0);
});
test('typed initial HELLO refusal frees slot but forged resolution quarantines it', async () => {
	const refused = await mockClient({ refusal: true });
	await assert.rejects(
		refused.open(),
		(e) => e.code === 'capability.unsupported',
	);
	assert.equal(refused.service.filterPreviewUsage().reserved, 0);
	assert.deepEqual(
		refused.messages.map((x) => x.message.kind),
		['hello'],
	);
	const forged = await mockClient({ tamper: true });
	await assert.rejects(forged.open());
	assert.equal(forged.service.filterPreviewUsage().quarantined, true);
	assert.ok(forged.service.filterPreviewUsage().reserved > 0);
	assert.deepEqual(
		forged.messages.map((x) => x.message.kind),
		['hello'],
	);
});
