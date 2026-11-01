// Source6 only. Contract2 metadata is never an execution grant.
import {
	BEGIN_BYTES_MAX,
	CAPABILITY_BYTES_MAX,
	encodeProgram,
	facts,
	freezeSource5,
	OPEN_BYTES_MAX,
	PROGRAM_CONTROL_BYTES,
	requirementDigests,
	sameProgram,
	validateFacts,
	validatePairs,
	validatePlan5,
	validateRequest5,
	validateResult5,
} from './filter-preview-program-protocol.js';
export const NOISE_CONTROL_BYTES = PROGRAM_CONTROL_BYTES;
const fail = (code, message) => Object.assign(new Error(message), { code });
const encoder = new TextEncoder();
const sourceKeys = [
	'owner',
	'draft',
	'document',
	'size',
	'layers',
	'residentBytes',
	'metadataBytes',
	'metadataCharge',
	'filterDigest',
	'maskCount',
	'maskBands',
	'maskRuns',
	'maskDecodeBytes',
];
function shape(v, keys, optional = []) {
	if (
		!v ||
		typeof v !== 'object' ||
		Array.isArray(v) ||
		Object.getPrototypeOf(v) !== Object.prototype
	)
		throw fail('InputSchema', 'Scoped object');
	const names = Object.keys(v);
	if (
		names.some((k) => !keys.includes(k) && !optional.includes(k)) ||
		keys.some((k) => !Object.hasOwn(v, k))
	)
		throw fail('InputSchema', 'Scoped object fields');
}
function digest(v) {
	if (typeof v !== 'string' || !/^[a-f0-9]{64}$/.test(v))
		throw fail('InputSchema', 'Scoped SHA');
}
function uint(v, max) {
	if (!Number.isSafeInteger(v) || v < 0 || v > max)
		throw fail('ResourceLimit', 'Scoped integer bound');
}
export function validateWorkload6(w) {
	shape(w, [
		'id',
		'revision',
		'workspaceBytes',
		'baseWorkLimit',
		'noiseWorkLimit',
		'totalWorkLimit',
		'sha256',
	]);
	if (w.id !== 'noise-source-work-v1' || w.revision !== 1)
		throw fail('CapabilityUnsupported', 'Unknown scoped workload');
	uint(w.workspaceBytes, 64 * 1024 * 1024);
	uint(w.baseWorkLimit, 268435456);
	uint(w.noiseWorkLimit, 34359738368);
	if (w.totalWorkLimit !== w.baseWorkLimit + w.noiseWorkLimit)
		throw fail('InputSchema', 'Scoped work sum');
	digest(w.sha256);
}
function bytes() {
	const parts = [];
	return {
		parts,
		text(s) {
			const b = encoder.encode(s);
			this.u16(b.length);
			parts.push(...b);
		},
		u16(n) {
			parts.push(n >>> 8, n & 255);
		},
		u32(n) {
			parts.push(n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255);
		},
		u64(n) {
			const v = BigInt(n);
			const b = new Uint8Array(8);
			new DataView(b.buffer).setBigUint64(0, v);
			parts.push(...b);
		},
		hash(s) {
			digest(s);
			for (let i = 0; i < 64; i += 2)
				parts.push(Number.parseInt(s.slice(i, i + 2), 16));
		},
		domain(s) {
			parts.push(...encoder.encode(s));
		},
	};
}
async function hash(b) {
	return [
		...new Uint8Array(
			await crypto.subtle.digest('SHA-256', Uint8Array.from(b)),
		),
	]
		.map((x) => x.toString(16).padStart(2, '0'))
		.join('');
}
export async function workloadDigest6(w) {
	validateWorkload6(w);
	const b = bytes();
	b.domain('paintcraft.noise.workload/v1\0');
	b.text(w.id);
	b.u32(w.revision);
	for (const n of [
		w.workspaceBytes,
		w.baseWorkLimit,
		w.noiseWorkLimit,
		w.totalWorkLimit,
	])
		b.u64(n);
	return hash(b.parts);
}
export function facts6(v) {
	return { ...facts(v), capabilitySha256: v.capabilitySha256 };
}
export function validateFacts6(v, max = 257, full = false) {
	shape(v, [
		'count',
		'requiredSha256',
		'resolutionSha256',
		'capabilitySha256',
		...(full ? ['pairs'] : []),
	]);
	const base = { ...v };
	delete base.capabilitySha256;
	validateFacts(base, max, full);
	digest(v.capabilitySha256);
}
function oldFact(v) {
	const value = { ...v };
	delete value.capabilitySha256;
	return value;
}
function oldPlan(plan) {
	return {
		...Object.fromEntries(
			[...sourceKeys, 'sourceSha256']
				.filter((k) => Object.hasOwn(plan, k))
				.map((k) => [k, plan[k]]),
		),
		protocol: 5,
		registryContract: 1,
		workspaceBytes: plan.workload.workspaceBytes,
		workLimit: plan.workload.baseWorkLimit,
		sourceRequirements: oldFact(plan.sourceRequirements),
	};
}
export function validatePlan6(plan, legacy) {
	shape(
		plan,
		[
			...sourceKeys,
			'protocol',
			'registryContract',
			'sourceRequirements',
			'workload',
		],
		['sourceSha256'],
	);
	if (plan.protocol !== 6 || plan.registryContract !== 2)
		throw fail('CapabilityUnsupported', 'Source6 contract');
	validateWorkload6(plan.workload);
	validateFacts6(plan.sourceRequirements, 256, true);
	validatePlan5(oldPlan(plan), legacy);
	encodeProgram(plan, OPEN_BYTES_MAX);
}
export function validateBinding6(binding, plan, legacy) {
	const source = { ...binding };
	delete source.candidateRequirements;
	delete source.unionRequirements;
	delete source.workloadSha256;
	validatePlan6(source, legacy);
	if (
		!sameProgram(source, plan) ||
		binding.workloadSha256 !== plan.workload.sha256
	)
		throw fail('CodecFailed', 'Source6 READY source');
	for (const key of ['candidateRequirements', 'unionRequirements']) {
		validateFacts6(binding[key], 256);
		if (!sameProgram(binding[key], facts6(plan.sourceRequirements)))
			throw fail('CodecFailed', 'Source6 READY facts');
	}
}
export function validateRequest6(options, plan, legacy, canonical) {
	shape(options, [
		'generation',
		'operation',
		'target',
		'view',
		'render',
		'candidateRequirements',
		'unionRequirements',
	]);
	validateFacts6(options.candidateRequirements, 256);
	validateFacts6(options.unionRequirements, 257);
	validateRequest5(
		{
			...options,
			candidateRequirements: oldFact(options.candidateRequirements),
			unionRequirements: oldFact(options.unionRequirements),
		},
		oldPlan(plan),
		legacy,
		canonical,
	);
	if (
		options.operation === null &&
		(!sameProgram(
			options.candidateRequirements,
			facts6(plan.sourceRequirements),
		) ||
			!sameProgram(
				options.unionRequirements,
				facts6(plan.sourceRequirements),
			))
	)
		throw fail('CodecFailed', 'Source6 BASE facts');
	encodeProgram(options, BEGIN_BYTES_MAX);
}
export function validateResult6(result, request, plan, legacy) {
	if (
		result.protocol !== 6 ||
		result.registryContract !== 2 ||
		result.workloadSha256 !== plan.workload.sha256
	)
		throw fail('CodecFailed', 'Source6 result contract');
	for (const key of [
		'sourceRequirements',
		'candidateRequirements',
		'unionRequirements',
	]) {
		validateFacts6(result[key], key === 'unionRequirements' ? 257 : 256);
		const expected =
			key === 'sourceRequirements'
				? facts6(plan.sourceRequirements)
				: request[key];
		if (!sameProgram(result[key], expected))
			throw fail('CodecFailed', 'Source6 result facts');
	}
	shape(result.workQuote, ['workloadSha256', 'base', 'noise', 'total']);
	uint(result.workQuote.base, plan.workload.baseWorkLimit);
	uint(result.workQuote.noise, plan.workload.noiseWorkLimit);
	if (
		result.workQuote.total !==
			result.workQuote.base + result.workQuote.noise ||
		result.workQuote.workloadSha256 !== plan.workload.sha256
	)
		throw fail('CodecFailed', 'Source6 result work');
	const old = { ...result };
	for (const k of [
		'protocol',
		'registryContract',
		'workloadSha256',
		'workQuote',
	])
		delete old[k];
	for (const k of [
		'sourceRequirements',
		'candidateRequirements',
		'unionRequirements',
	])
		old[k] = oldFact(old[k]);
	validateResult5(
		old,
		{
			...request,
			candidateRequirements: oldFact(request.candidateRequirements),
			unionRequirements: oldFact(request.unionRequirements),
		},
		oldPlan(plan),
		legacy,
	);
}
export async function capabilityFacts6(pairs, entries, w) {
	validatePairs(pairs);
	validateWorkload6(w);
	if (!Array.isArray(entries) || entries.length !== pairs.length)
		throw fail('CodecFailed', 'Source6 entries');
	for (const e of entries) {
		shape(e, [
			'profile_id',
			'codec_id',
			'resource_contract_id',
			'resource_contract_revision',
			'workload_class_id',
			'workload_class_revision',
		]);
		if (
			!['base-work-v1', 'noise-source-work-v1'].includes(
				e.workload_class_id,
			) ||
			e.workload_class_revision !== 1
		)
			throw fail('CapabilityUnsupported', 'Unknown scoped class');
	}
	const base = await requirementDigests(
		pairs,
		entries.map((e) => ({
			profile_id: e.profile_id,
			codec_id: e.codec_id,
			resource_contract_id: e.resource_contract_id,
			resource_contract_revision: e.resource_contract_revision,
		})),
	);
	if ((await workloadDigest6(w)) !== w.sha256)
		throw fail('CodecFailed', 'Scoped workload digest');
	const b = bytes();
	b.domain('paintcraft.worker-capabilities/v2\0');
	b.u16(2);
	b.hash(base.requiredSha256);
	b.hash(base.resolutionSha256);
	b.hash(w.sha256);
	b.u16(pairs.length);
	for (let i = 0; i < pairs.length; i++) {
		b.text(pairs[i].profile_id);
		b.text(pairs[i].codec_id);
		b.text(entries[i].workload_class_id);
		b.u32(entries[i].workload_class_revision);
	}
	return { ...base, capabilitySha256: await hash(b.parts) };
}
export function validateRuntimeFacts6(v) {
	shape(v, ['controlQuote', 'slicePolicy']);
	shape(v.controlQuote, [
		'retainedBytes',
		'handleBytes',
		'frameBytes',
		'extraOutputBytes',
	]);
	for (const k of ['retainedBytes', 'handleBytes', 'frameBytes']) {
		uint(v.controlQuote[k], (k === 'handleBytes' ? 32 : 64) * 1024 * 1024);
		if (!v.controlQuote[k]) throw fail('CodecFailed', 'Source6 ABI quote');
	}
	if (
		v.controlQuote.frameBytes > v.controlQuote.retainedBytes ||
		v.controlQuote.extraOutputBytes !== 4_194_304 * 4
	)
		throw fail('CodecFailed', 'Source6 ABI quote');
	shape(v.slicePolicy, ['nativeEdge', 'baseSliceLimit', 'noiseSliceLimit']);
	if (
		v.slicePolicy.nativeEdge !== 64 ||
		v.slicePolicy.baseSliceLimit !== 8388608 ||
		v.slicePolicy.noiseSliceLimit !== 134217728
	)
		throw fail('CapabilityUnsupported', 'Source6 fixed slice policy');
	encodeProgram(v, CAPABILITY_BYTES_MAX);
}
export async function validateCapabilities6(reply, pairs, expected, w) {
	shape(reply, [
		'kind',
		'id',
		'protocol',
		'registryContract',
		'entries',
		'requiredSha256',
		'resolutionSha256',
		'workloadSha256',
		'capabilitySha256',
		'runtimeFacts',
	]);
	if (
		reply.kind !== 'capabilities' ||
		reply.protocol !== 6 ||
		reply.registryContract !== 2 ||
		!Number.isInteger(reply.id) ||
		reply.id < 1 ||
		reply.id > 0xffffffff
	)
		throw fail('CodecFailed', 'Source6 HELLO2');
	validateRuntimeFacts6(reply.runtimeFacts);
	const actual = await capabilityFacts6(pairs, reply.entries, w);
	encodeProgram(reply, CAPABILITY_BYTES_MAX);
	if (
		!sameProgram(actual, facts6(expected)) ||
		reply.requiredSha256 !== actual.requiredSha256 ||
		reply.resolutionSha256 !== actual.resolutionSha256 ||
		reply.capabilitySha256 !== actual.capabilitySha256 ||
		reply.workloadSha256 !== w.sha256
	)
		throw fail('CodecFailed', 'Source6 HELLO2 binding');
	return reply.entries.map((e) => Object.freeze({ ...e }));
}
export function freezeSource6(binding) {
	freezeSource5(binding);
	Object.freeze(binding.workload);
}
export function validateRuntimeRequest6(request, plan, legacy, canonical) {
	shape(request, [
		'generation',
		'operation',
		'target',
		'view',
		'render',
		'candidateRequirements',
		'unionRequirements',
		'owner',
		'draft',
		'sourceSha256',
		'protocol',
		'registryContract',
		'workloadSha256',
	]);
	if (
		request.protocol !== 6 ||
		request.registryContract !== 2 ||
		request.workloadSha256 !== plan.workload.sha256
	)
		throw fail('ConflictStaleBase', 'Source6 runtime workload');
	for (const key of ['owner', 'draft', 'sourceSha256'])
		if (!sameProgram(request[key], plan[key]))
			throw fail('ConflictStaleBase', 'Source6 runtime owner');
	const options = Object.fromEntries(
		[
			'generation',
			'operation',
			'target',
			'view',
			'render',
			'candidateRequirements',
			'unionRequirements',
		].map((k) => [k, request[k]]),
	);
	validateRequest6(options, plan, legacy, canonical);
	encodeProgram(request, BEGIN_BYTES_MAX);
}

// Raw Worker transport is untrusted even though the admitted Main also checks it.
export function cancelGeneration6(value) {
	if (typeof value !== 'string' || !/^(0|[1-9]\d{0,19})$/.test(value))
		throw fail('InputSchema', 'Source6 cancel generation');
	const generation = BigInt(value);
	if (generation > 0xffffffffffffffffn)
		throw fail('InputSchema', 'Source6 cancel generation');
	return generation;
}
