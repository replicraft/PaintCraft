// Synthetic metadata fixtures only: no real Worker/ABI/kernel claim.
import {
	APPEARANCE7,
	admissionDigest7,
	capabilityDigest7,
	facts7,
	geometry7,
	numericalDigests7,
	numericalEntry7,
	numericalFacts7,
	operationDigest7,
	slice7,
	unionDigest7,
} from '../src/lib/filter-preview-appearance-protocol.js';
const grant = (noise = false) => ({
	mode: noise ? 'noise' : 'base',
	workspaceBytes: '67108864',
	baseWorkLimit: '268435456',
	noiseWorkLimit: noise ? '34359738368' : '0',
	totalWorkLimit: noise ? '34628173824' : '268435456',
});
export const owner = {
	session: '11111111-1111-1111-1111-111111111111',
	state: '22222222-2222-2222-2222-222222222222',
	epoch: '0',
	frontend: 3,
};
export const draft = '33333333-3333-3333-3333-333333333333';
const target = '55555555-5555-5555-5555-555555555555';
export const runtime = (noise = false) => ({
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
export async function plan(profiles = ['invert-rgb-v1'], noise = false) {
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
export async function begin(p, render = false) {
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
export async function binding(p, b) {
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
