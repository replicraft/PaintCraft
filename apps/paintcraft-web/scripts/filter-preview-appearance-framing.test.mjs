import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import {
	admissionDigest7,
	capabilityDigest7,
	unionDigest7,
	validateLayerMasks7,
} from '../src/lib/filter-preview-appearance-protocol.js';
globalThis.crypto ??= webcrypto;
const oracle = JSON.parse(
	await readFile(
		new URL(
			'../../../crates/services/paintcraft-formats/tests/source7-framing-fixtures/framing-vectors.json',
			import.meta.url,
		),
		'utf8',
	),
);
const admission = (noise = false) => ({
	mode: noise ? 'noise' : 'base',
	workAuthority: 'captured-job',
	workspaceBytes: '67108864',
	baseWorkLimit: '268435456',
	noiseWorkLimit: noise ? '34359738368' : '0',
	totalWorkLimit: noise ? '34628173824' : '268435456',
});
class Reader {
	constructor(hex) {
		this.data = Uint8Array.from(Buffer.from(hex, 'hex'));
		this.at = this.data.indexOf(0) + 1;
	}
	take(n) {
		const b = this.data.slice(this.at, this.at + n);
		assert.equal(b.length, n);
		this.at += n;
		return b;
	}
	n(width) {
		let n = 0n;
		for (const b of this.take(width)) n = (n << 8n) | BigInt(b);
		return n;
	}
	hash() {
		return Buffer.from(this.take(32)).toString('hex');
	}
	text() {
		return new TextDecoder().decode(this.take(Number(this.n(2))));
	}
	prefix() {
		assert.equal(this.n(2), 3n);
		const base = {
			count: 0,
			requiredSha256: this.hash(),
			resolutionSha256: this.hash(),
		};
		const grant = this.hash();
		assert.equal(this.text(), 'layer-masks.v1');
		assert.equal(this.text(), 'layer-mask-final-alpha-rgba8-v1');
		assert.equal(this.n(2), 1n);
		assert.equal(this.n(2), 1n);
		assert.equal(this.n(2), 1n);
		assert.equal(this.text(), 'base-work-v1');
		assert.equal(this.n(2), 1n);
		return { base, grant };
	}
	fact(expected) {
		const { base, grant } = this.prefix();
		const own = {
			stored: Number(this.n(4)),
			enabled: Number(this.n(4)),
			bands: Number(this.n(8)),
			runs: Number(this.n(8)),
			decodeUpperBytes: this.n(8).toString(),
			identitySha256: this.hash(),
			coverageQuoteRevision: 1,
			coverageAbi: 'wasm32-v1',
		};
		const fact = {
			...base,
			layerMasks: own,
			sourceCoverageSha256: this.hash(),
			maskInventorySha256: this.hash(),
			capabilitySha256: expected,
		};
		assert.equal(this.at, this.data.length);
		return { fact, grant };
	}
	union(expected) {
		const { base, grant } = this.prefix();
		const fact = {
			...base,
			sourceCoverageSha256: this.hash(),
			capabilitySha256: expected,
		};
		assert.equal(this.at, this.data.length);
		return { fact, grant };
	}
}
test('Source7 SHA matches all independently frozen admission and source/candidate/union literal frames', async () => {
	let captures = 0;
	for (const row of oracle.vectors) {
		assert.equal(
			await admissionDigest7(admission(row.noise)),
			row.frames.admission.sha256,
		);
		const expected = row.frames.capability,
			{ fact, grant } = new Reader(expected.hex).fact(expected.sha256);
		assert.equal(grant, row.frames.admission.sha256);
		assert.equal(
			await capabilityDigest7(fact, admission(row.noise)),
			expected.sha256,
		);
		captures++;
	}
	for (const row of oracle.candidateMutationVectors) {
		const c = new Reader(row.candidateCapability.hex).fact(
			row.candidateCapability.sha256,
		);
		assert.equal(
			await capabilityDigest7(c.fact, admission()),
			row.candidateCapability.sha256,
		);
		const u = new Reader(row.unionCapability.hex).union(
			row.unionCapability.sha256,
		);
		assert.equal(
			await unionDigest7(u.fact, admission()),
			row.unionCapability.sha256,
		);
		captures += 2;
	}
	assert.equal(captures, 30);
});
test('Source7 candidate inventories may differ while immutable source coverage and capability union stay fixed', async () => {
	for (const name of [
		'active-filter-mask-source',
		'disabled-filter-mask-source',
		'initially-disabled-filter-enabled-mask',
	]) {
		const rows = oracle.candidateMutationVectors.filter((v) =>
			v.name.startsWith(`${name}/`),
		);
		assert.equal(rows.length, 4);
		const facts = rows.map(
			(v) =>
				new Reader(v.candidateCapability.hex).fact(
					v.candidateCapability.sha256,
				).fact,
		);
		assert.equal(new Set(facts.map((f) => f.maskInventorySha256)).size, 4);
		assert.equal(new Set(facts.map((f) => f.sourceCoverageSha256)).size, 1);
		assert.equal(
			new Set(rows.map((v) => v.unionCapability.sha256)).size,
			1,
		);
	}
});
test('Source7 u32 own counts have no artificial u16 acceptance ceiling and browser ABI is closed', () => {
	const own = {
		stored: 65536,
		enabled: 65536,
		bands: 0,
		runs: 0,
		decodeUpperBytes: '0',
		identitySha256: '00'.repeat(32),
		coverageQuoteRevision: 1,
		coverageAbi: 'wasm32-v1',
	};
	validateLayerMasks7(own);
	assert.equal(oracle.count65536_u32be, '00010000');
	assert.throws(
		() => validateLayerMasks7({ ...own, coverageAbi: 'native64-v1' }),
		{ code: 'CapabilityUnsupported' },
	);
	assert.throws(() => validateLayerMasks7({ ...own, stored: 100001 }), {
		code: 'InputSchema',
	});
});

test('Source7 admission v2 origin changes admission, source capability and union while old numeric inputs stay fixed', async () => {
	const admissionHashes = new Set(),
		capabilities = new Set(),
		unions = new Set();
	for (const row of oracle.originVectors) {
		const a = { ...admission(row.noise), workAuthority: row.workAuthority };
		assert.equal(await admissionDigest7(a), row.frames.admission.sha256);
		const c = new Reader(row.frames.capability.hex).fact(
			row.frames.capability.sha256,
		);
		assert.equal(
			await capabilityDigest7(c.fact, a),
			row.frames.capability.sha256,
		);
		const u = new Reader(row.frames.union.hex).union(
			row.frames.union.sha256,
		);
		assert.equal(await unionDigest7(u.fact, a), row.frames.union.sha256);
		admissionHashes.add(row.frames.admission.sha256);
		capabilities.add(row.frames.capability.sha256);
		unions.add(row.frames.union.sha256);
	}
	assert.equal(admissionHashes.size, 4);
	assert.equal(capabilities.size, 4);
	assert.equal(unions.size, 4);
});
