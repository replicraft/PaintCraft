import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';
import {
	APPEARANCE7,
	admissionDigest7,
	capabilityDigest7,
	equal7,
	facts7,
	geometry7,
	numericalDigests7,
	numericalEntry7,
	numericalFacts7,
	operationDigest7,
	slice7,
	unionDigest7,
	validateAdmission7,
	validateBegin7,
	validateBinding7,
	validateCapabilities7,
	validateCapabilitiesMessage7,
	validateContext7,
	validateCopyAdmitted7,
	validateDescriptor7,
	validateFacts7,
	validatePlan7,
	validatePreplan7,
	validateReady7,
	validateRuntimeFacts7,
	validateUnion7,
	validateWorkQuote7,
	verifyBegin7,
	verifyBinding7,
	verifyPlan7,
} from '../src/lib/filter-preview-appearance-protocol.js';
import { validatePlan5 } from '../src/lib/filter-preview-program-protocol.js';
import { validatePlan6 } from '../src/lib/filter-preview-noise-protocol.js';
globalThis.crypto ??= webcrypto;
// Synthetic metadata/work extents exercise grammar, not actual Rust ABI admission.
const grant = (noise = false) => ({
	mode: noise ? 'noise' : 'base',
	workspaceBytes: '67108864',
	baseWorkLimit: '268435456',
	noiseWorkLimit: noise ? '34359738368' : '0',
	totalWorkLimit: noise ? '34628173824' : '268435456',
});
const owner = {
	session: '11111111-1111-1111-1111-111111111111',
	state: '22222222-2222-2222-2222-222222222222',
	epoch: '0',
	frontend: 3,
};
const draft = '33333333-3333-3333-3333-333333333333';
const target = '55555555-5555-5555-5555-555555555555';
const runtime = (noise = false) => ({
	protocol: 7,
	registryContract: 3,
	appearanceRevision: 1,
	queryRevision: 1,
	coverageAbi: 'wasm32-v1',
	sliceRevision: 1,
	nativeEdge: 64,
	baseSliceWork: '8388608',
	noiseSliceWork: noise ? '134217728' : '0',
	controlQuoteBytes: '1048576',
});
async function plan(profiles = ['invert-rgb-v1'], noise = false) {
	const pairs = profiles.map((profile_id) => ({
			profile_id,
			codec_id: 'params-v1',
		})),
		entries = pairs.map(numericalEntry7),
		admission = grant(noise);
	const numerical = await numericalDigests7(pairs, entries);
	const fact = {
		...numerical,
		layerMasks: {
			stored: 1,
			enabled: 1,
			bands: 1,
			runs: 1,
			decodeUpperBytes: '128',
			identitySha256: 'aa'.repeat(32),
			coverageQuoteRevision: 1,
			coverageAbi: 'wasm32-v1',
		},
		sourceCoverageSha256: 'bb'.repeat(32),
		maskInventorySha256: 'cc'.repeat(32),
		capabilitySha256: '00'.repeat(32),
	};
	fact.capabilitySha256 = await capabilityDigest7(fact, admission);
	return {
		protocol: 7,
		registryContract: 3,
		owner: { ...owner },
		draft,
		document: '44444444-4444-4444-4444-444444444444',
		sourceSha256: '11'.repeat(32),
		size: [5, 3],
		layers: 1,
		residentBytes: 0,
		metadataBytes: 0,
		metadataCharge: 0,
		filterDigest: '22'.repeat(32),
		maskCount: 0,
		maskBands: 0,
		maskRuns: 0,
		maskDecodeBytes: 0,
		sourceRequirements: { ...fact, pairs },
		appearance: { ...APPEARANCE7 },
		admission,
		slice: slice7(admission.mode),
	};
}
async function begin(p, render = false) {
	const union = {
		...numericalFacts7(p.sourceRequirements),
		sourceCoverageSha256: p.sourceRequirements.sourceCoverageSha256,
		capabilitySha256: '00'.repeat(32),
	};
	union.capabilitySha256 = await unionDigest7(union, p.admission);
	return {
		protocol: 7,
		registryContract: 3,
		owner: { ...p.owner },
		draft: p.draft,
		sourceSha256: p.sourceSha256,
		generation: '1',
		operation: null,
		target: null,
		view: { region: [1, 1, 1, 1], level: 1 },
		render,
		candidateRequirements: facts7(p.sourceRequirements),
		unionRequirements: union,
		admissionSha256: await admissionDigest7(p.admission),
	};
}
async function binding(p, b) {
	const g = geometry7(b.view, p.size);
	return {
		kind: b.render ? 'frame' : 'prepared',
		protocol: 7,
		registryContract: 3,
		generation: b.generation,
		owner: structuredClone(b.owner),
		draft: b.draft,
		sourceSha256: b.sourceSha256,
		target: b.target,
		view: structuredClone(b.view),
		...g,
		operationSha256: await operationDigest7(b.operation),
		admissionSha256: b.admissionSha256,
		sourceRequirements: facts7(p.sourceRequirements),
		candidateRequirements: structuredClone(b.candidateRequirements),
		unionRequirements: structuredClone(b.unionRequirements),
		workQuote: {
			baseWork: '1000',
			noiseWork: '0',
			totalWork: '1000',
			maskWorkActual: '48',
			maskWorkUpper: '64',
			maskWorkMargin: '16',
		},
	};
}
test('Source7 filterless masks require independent appearance and exact preplan/final selectors', async () => {
	const p = await plan([]);
	validatePlan7(p);
	validatePreplan7({
		sourceProtocol: 7,
		registryContract: 3,
		mode: 'base',
		owner,
		draft,
	});
	assert.throws(
		() =>
			validatePreplan7({
				protocol: 7,
				registryContract: 3,
				mode: 'base',
				owner,
				draft,
			}),
		{ code: 'InputSchema' },
	);
	for (const q of [
		{ ...p, appearance: undefined },
		{
			...p,
			sourceRequirements: {
				...p.sourceRequirements,
				layerMasks: {
					...p.sourceRequirements.layerMasks,
					stored: 0,
					enabled: 0,
				},
			},
		},
		{ ...p, registryContract: 2 },
		{ ...p, future: 1 },
	])
		assert.throws(() => validatePlan7(q));
	assert.throws(
		() =>
			validatePlan7({
				...p,
				owner: {
					...p.owner,
					session: p.owner.session
						.toUpperCase()
						.replace('11111111', 'AAAAAAAA'),
				},
			}),
		{ code: 'InputSchema' },
	);
});
test('Source7 stored Noise requires paired mode even when latent; prospective Noise mode is allowed without stored Noise', async () => {
	const noise = await plan(['add-noise-rgb-v1'], true);
	validatePlan7(noise);
	assert.throws(
		() =>
			validatePlan7({
				...noise,
				admission: grant(),
				slice: slice7('base'),
			}),
		{ code: 'CapabilityUnsupported' },
	);
	const prospective = await plan([], true);
	validatePlan7(prospective);
	for (const bad of ['01', '1.0', '-0', 1, '18446744073709551616'])
		assert.throws(() =>
			validateAdmission7({ ...grant(), baseWorkLimit: bad }),
		);
	assert.throws(
		() =>
			validateAdmission7(grant(), { ...slice7('base'), nativeEdge: 32 }),
		{ code: 'CapabilityUnsupported' },
	);
});
test('Source7 facts and capability-only union have separate exact shapes and no summed inventory allowance', async () => {
	const p = await plan(),
		b = await begin(p);
	validateFacts7(p.sourceRequirements, true);
	validateUnion7(b.unionRequirements);
	assert.notEqual(
		b.unionRequirements.capabilitySha256,
		b.candidateRequirements.capabilitySha256,
	);
	assert.throws(
		() =>
			validateUnion7({
				...b.unionRequirements,
				layerMasks: p.sourceRequirements.layerMasks,
			}),
		{ code: 'InputSchema' },
	);
	assert.throws(
		() => validateFacts7({ ...b.candidateRequirements, pairs: [] }),
		{ code: 'InputSchema' },
	);
	validateBegin7(b, p);
	await verifyBegin7(b, p);
	assert.throws(
		() =>
			validateBegin7(
				{
					...b,
					candidateRequirements: {
						...b.candidateRequirements,
						sourceCoverageSha256: 'dd'.repeat(32),
					},
				},
				p,
			),
		{ code: 'ConflictStaleBase' },
	);
});
test('Source7 requested HELLO subset binds exact resources/classes/expected runtime facts and transport id remains separate', async () => {
	const p = await plan(),
		pairs = p.sourceRequirements.pairs,
		entries = pairs.map(numericalEntry7);
	const reply = {
		kind: 'capabilities',
		protocol: 7,
		registryContract: 3,
		entries,
		appearance: { ...APPEARANCE7 },
		admissionSha256: await admissionDigest7(p.admission),
		...numericalFacts7(p.sourceRequirements),
		runtimeFacts: runtime(),
	};
	delete reply.count;
	const actual = await validateCapabilities7(
		reply,
		pairs,
		numericalFacts7(p.sourceRequirements),
		p.admission,
		runtime(),
	);
	assert.equal(actual.length, 1);
	assert.ok(Object.isFrozen(actual[0]));
	await validateCapabilitiesMessage7(
		{ id: 1, ...reply },
		pairs,
		numericalFacts7(p.sourceRequirements),
		p.admission,
		runtime(),
	);
	for (const q of [
		{ ...reply, id: 1 },
		{ ...reply, entries: [...entries, entries[0]] },
		{
			...reply,
			entries: [
				{ ...entries[0], workload_class_id: 'noise-source-work-v1' },
			],
		},
		{
			...reply,
			runtimeFacts: { ...runtime(), coverageAbi: 'native64-v1' },
		},
		{ ...reply, admissionSha256: '00'.repeat(32) },
	])
		await assert.rejects(
			validateCapabilities7(
				q,
				pairs,
				numericalFacts7(p.sourceRequirements),
				p.admission,
				runtime(),
			),
		);
	validateReady7(
		{ runtimeFacts: runtime(), controlQuote: runtime().controlQuoteBytes },
		'base',
	);
	assert.throws(
		() =>
			validateReady7(
				{ runtimeFacts: runtime(), controlQuote: '1' },
				'base',
			),
		{ code: 'CodecFailed' },
	);
	assert.throws(() => validateRuntimeFacts7(runtime(true), 'base'), {
		code: 'CapabilityUnsupported',
	});
});
test('Source7 validates exact quantized ROI and prepared nonzero work is a prediction rather than a debit receipt', async () => {
	const p = await plan(),
		b = await begin(p),
		out = await binding(p, b);
	validateBinding7(out, b, p);
	await verifyBinding7(out, b, p);
	assert.deepEqual(out.region, [0, 0, 2, 2]);
	assert.equal(out.kind, 'prepared');
	assert.notEqual(out.workQuote.baseWork, '0');
	assert.throws(
		() => validateBinding7({ ...out, region: [1, 1, 1, 1] }, b, p),
		{ code: 'CodecFailed' },
	);
	assert.throws(
		() =>
			validateWorkQuote7(
				{ ...out.workQuote, maskWorkMargin: '17' },
				p.admission,
			),
		{ code: 'CodecFailed' },
	);
	assert.throws(
		() =>
			validateBinding7(
				{ ...out, owner: { ...out.owner, epoch: '1' } },
				b,
				p,
			),
		{ code: 'CodecFailed' },
	);
	assert.throws(
		() =>
			validateBinding7(
				{
					...out,
					sourceRequirements: {
						...out.sourceRequirements,
						maskInventorySha256: 'dd'.repeat(32),
					},
				},
				b,
				p,
			),
		{ code: 'CodecFailed' },
	);
	validateContext7(p, p);
	validateContext7(out, p, b);
	const copyBegin = {
			...b,
			render: true,
			view: { region: [1, 1, 1, 1], level: 0 },
		},
		copy = {
			kind: 'copy-admitted',
			outputVersion: 1,
			outputSha256: '12'.repeat(32),
			binding: await binding(p, copyBegin),
		};
	validateCopyAdmitted7(copy, copyBegin, p, '12'.repeat(32));
	for (const invalid of [
		b,
		{ ...copyBegin, render: false },
		{ ...copyBegin, view: b.view },
		{ ...copyBegin, operation: {} },
	])
		assert.throws(
			() => validateCopyAdmitted7(copy, invalid, p, '12'.repeat(32)),
			{ code: 'CapabilityUnsupported' },
		);
});
test('Source7 numeric operation grammar closes masks/aliases/unknown fields and checked parameter domains', async () => {
	const p = await plan(),
		b = await begin(p),
		operation = {
			op: 'filter.update',
			layer_ref: { id: target },
			filter_ref: { id: '66666666-6666-6666-6666-666666666666' },
			descriptor: {
				kind: 'levels-rgb-v1',
				input_black: 16,
				input_white: 240,
				gamma_hundredths: 999,
				output_black: 255,
				output_white: 0,
			},
		};
	validateDescriptor7(operation.descriptor);
	for (const descriptor of [
		{ ...operation.descriptor, gamma_hundredths: 9 },
		{ ...operation.descriptor, input_white: 16 },
		{ kind: 'curves-rgb-v1', outputs: [0, 1, 2, 3, 256] },
		{ kind: 'box-blur-v1', radius: 33 },
		{ kind: 'unknown-v1' },
	])
		assert.throws(() => validateDescriptor7(descriptor));
	assert.throws(
		() =>
			validateBegin7(
				{
					...b,
					operation: {
						...operation,
						descriptor: {
							kind: 'add-noise-rgb-v1',
							amount_hundredths: 0,
							seed: 65535,
							monochrome: true,
							distribution: 'gaussian',
						},
					},
					target,
				},
				p,
			),
		{ code: 'CapabilityUnsupported' },
	);
	for (const bad of [
		{ ...operation, extra: 1 },
		{ op: 'filter.mask.remove', layer_ref: { id: target } },
		{ ...operation, layer_ref: { ref: 'alias' } },
	])
		assert.throws(() =>
			validateBegin7({ ...b, operation: bad, target }, p),
		);
});
test('Source7 async hash verification detaches every input before first await', {
	concurrency: false,
}, async () => {
	const p = await plan(),
		entries = p.sourceRequirements.pairs.map(numericalEntry7),
		old = structuredClone(p);
	const original = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
	let entered, release;
	const waiting = new Promise((r) => {
			entered = r;
		}),
		gate = new Promise((r) => {
			release = r;
		});
	Object.defineProperty(globalThis, 'crypto', {
		configurable: true,
		value: {
			subtle: {
				digest(...args) {
					entered();
					return gate.then(() => webcrypto.subtle.digest(...args));
				},
			},
		},
	});
	try {
		const pending = verifyPlan7(p, entries);
		await waiting;
		p.sourceRequirements.layerMasks.identitySha256 = 'ff'.repeat(32);
		entries[0].resource_contract_id = 'changed-v1';
		release();
		const captured = await pending;
		assert.ok(equal7(captured, old));
		assert.ok(Object.isFrozen(captured.sourceRequirements.layerMasks));
	} finally {
		release();
		if (original) Object.defineProperty(globalThis, 'crypto', original);
		else delete globalThis.crypto;
	}
});
test('Source7 bounded encoding rejects hostile controls before async work or toJSON and old5/6 stay closed', async () => {
	const p = await plan();
	let invoked = 0;
	assert.throws(() =>
		validatePlan7({
			...p,
			toJSON() {
				invoked++;
				return {};
			},
		}),
	);
	assert.equal(invoked, 0);
	const giant = { ...p, filterDigest: 'x'.repeat(1024 * 1024) };
	await assert.rejects(verifyPlan7(giant, []), { code: 'ResourceLimit' });
	assert.throws(() => validatePlan5(p, () => {}));
	assert.throws(() => validatePlan6(p, () => {}));
});

test('Source7 current13 descriptor shapes resolve with old12 numerical digests byte-identical', async () => {
	const descriptors = [
		{ kind: 'invert-rgb-v1' },
		{ kind: 'brightness-contrast-v1', brightness: 0, contrast: 0 },
		{ kind: 'black-white-rgb-v1', strength: 100 },
		{ kind: 'threshold-luma-v1', level: 128 },
		{ kind: 'posterize-rgb-v1', levels: 6 },
		{ kind: 'solarize-rgb-v1' },
		{ kind: 'box-blur-v1', radius: 0 },
		{
			kind: 'levels-rgb-v1',
			input_black: 0,
			input_white: 255,
			gamma_hundredths: 100,
			output_black: 0,
			output_white: 255,
		},
		{ kind: 'exposure-srgb-v1', stops_hundredths: 0 },
		{ kind: 'curves-rgb-v1', outputs: [0, 64, 128, 192, 255] },
		{ kind: 'vibrance-rgb-v1', amount_hundredths: 0 },
		{
			kind: 'hue-saturation-rgb-v1',
			hue_degrees: 0,
			saturation_hundredths: 0,
			lightness_hundredths: 0,
		},
		{
			kind: 'add-noise-rgb-v1',
			amount_hundredths: 10,
			seed: 1,
			monochrome: true,
			distribution: 'uniform',
		},
	];
	for (const d of descriptors) validateDescriptor7(d);
	const pairs = descriptors
		.slice(0, 12)
		.map((d) => ({ profile_id: d.kind, codec_id: 'params-v1' }))
		.sort((a, b) => (a.profile_id < b.profile_id ? -1 : 1));
	const actual = await numericalDigests7(pairs, pairs.map(numericalEntry7));
	// Existing frozen Rust old12 regression literals, not derived from this JS.
	assert.equal(
		actual.requiredSha256,
		'585a38dbdd4ff739357095aba8ca932ef90785942361e7335b772de331e553c1',
	);
	assert.equal(
		actual.resolutionSha256,
		'19c1306ec948ec8a0ae131be6fe4418e8bb9fc805b41c3bbfbbb6d4bd0539c1b',
	);
	assert.throws(
		() =>
			numericalEntry7({
				profile_id: 'layer-masks-v1',
				codec_id: 'params-v1',
			}),
		{ code: 'CapabilityUnsupported' },
	);
	assert.throws(
		() =>
			numericalEntry7({
				profile_id: 'invert-rgb-v1',
				codec_id: 'params-v2',
			}),
		{ code: 'CapabilityUnsupported' },
	);
});
