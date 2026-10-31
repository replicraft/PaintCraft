// Source5 only. No list of reader-known algorithms grants execution support.
export const OPEN_BYTES_MAX = 64 * 1024;
export const CAPABILITY_BYTES_MAX = 128 * 1024;
export const BEGIN_BYTES_MAX = 8 * 1024;
// The SAME admitted slot owns these bounded metadata copies; no extra lease.
export const PROGRAM_CONTROL_BYTES =
	4 * OPEN_BYTES_MAX + 4 * CAPABILITY_BYTES_MAX + 4 * BEGIN_BYTES_MAX;
const fail = (code, message) => Object.assign(new Error(message), { code });
const encoder = new TextEncoder();
function shape(v, keys, optional = []) {
	if (
		!v ||
		typeof v !== 'object' ||
		Array.isArray(v) ||
		Object.getPrototypeOf(v) !== Object.prototype
	)
		throw fail('InputSchema', 'Prepared control object');
	const names = Object.keys(v);
	if (
		names.length > keys.length + optional.length ||
		names.some((k) => !keys.includes(k) && !optional.includes(k)) ||
		keys.some((k) => !Object.hasOwn(v, k))
	)
		throw fail('InputSchema', 'Prepared control fields');
}
export function encodeProgram(value, limit) {
	if (
		!Number.isSafeInteger(limit) ||
		limit < 0 ||
		limit > CAPABILITY_BYTES_MAX
	)
		throw fail('InputSchema', 'Prepared byte limit');
	let nodes = 0,
		remaining = limit;
	const parts = [];
	let chunk = '';
	function append(text) {
		// Every appended token is ASCII. Strings are emitted scalar by scalar,
		// so no aggregate serialization or UTF8 buffer exists before admission.
		if (text.length > remaining)
			throw fail('ResourceLimit', 'Prepared control byte extent');
		remaining -= text.length;
		if (chunk.length + text.length > 256) {
			parts.push(chunk);
			chunk = '';
		}
		chunk += text;
	}
	function string(text) {
		append('"');
		for (let i = 0; i < text.length; i++) {
			const code = text.charCodeAt(i);
			if (code === 34 || code === 92) append(`\\${text[i]}`);
			else if (code < 32 || code >= 127) {
				// Canonical JSON escapes also preserve lone surrogates exactly.
				append(`\\u${code.toString(16).padStart(4, '0')}`);
			} else append(text[i]);
		}
		append('"');
	}
	function visit(v, depth) {
		if (++nodes > 4096 || depth > 8)
			throw fail('InputSchema', 'Prepared control nesting');
		if (
			v === null ||
			typeof v === 'boolean' ||
			(typeof v === 'number' && Number.isSafeInteger(v))
		) {
			append(JSON.stringify(v));
			return;
		}
		if (typeof v === 'string') {
			string(v);
			return;
		}
		if (Array.isArray(v)) {
			if (v.length > 257)
				throw fail('InputSchema', 'Prepared control array');
			append('[');
			for (let i = 0; i < v.length; i++) {
				if (i) append(',');
				visit(v[i], depth + 1);
			}
			append(']');
			return;
		}
		if (
			!v ||
			typeof v !== 'object' ||
			Object.getPrototypeOf(v) !== Object.prototype
		)
			throw fail('InputSchema', 'Prepared control value');
		const keys = Object.keys(v);
		if (keys.length > 32 || keys.some((k) => k.length > 64))
			throw fail('InputSchema', 'Prepared control object');
		append('{');
		for (const [i, key] of keys.sort().entries()) {
			if (i) append(',');
			string(key);
			append(':');
			visit(v[key], depth + 1);
		}
		append('}');
	}
	visit(value, 0);
	parts.push(chunk);
	return parts.join('');
}
export const sameProgram = (a, b) =>
	encodeProgram(a, CAPABILITY_BYTES_MAX) ===
	encodeProgram(b, CAPABILITY_BYTES_MAX);
function id(v, max) {
	if (
		typeof v !== 'string' ||
		v.length < 1 ||
		v.length > max ||
		!/^([a-z][a-z0-9-]*)-v[1-9][0-9]*$/.test(v)
	)
		throw fail('InputSchema', 'Prepared identifier');
}
const compare = (a, b) =>
	a.profile_id < b.profile_id
		? -1
		: a.profile_id > b.profile_id
			? 1
			: a.codec_id < b.codec_id
				? -1
				: a.codec_id > b.codec_id
					? 1
					: 0;
export function validatePairs(pairs, max = 257) {
	if (!Array.isArray(pairs) || pairs.length > max)
		throw fail('ResourceLimit', 'Prepared requirement count');
	for (let i = 0; i < pairs.length; i++) {
		const p = pairs[i];
		shape(p, ['profile_id', 'codec_id']);
		id(p.profile_id, 96);
		id(p.codec_id, 64);
		if (i && compare(pairs[i - 1], p) >= 0)
			throw fail('InputSchema', 'Prepared pairs must be sorted unique');
	}
}
function digest(v) {
	if (typeof v !== 'string' || !/^[a-f0-9]{64}$/.test(v))
		throw fail('InputSchema', 'Prepared digest');
}
export function validateFacts(v, max = 257, full = false) {
	shape(v, [
		'count',
		'requiredSha256',
		'resolutionSha256',
		...(full ? ['pairs'] : []),
	]);
	if (!Number.isSafeInteger(v.count) || v.count < 0 || v.count > max)
		throw fail('ResourceLimit', 'Prepared inventory count');
	digest(v.requiredSha256);
	digest(v.resolutionSha256);
	if (full) {
		validatePairs(v.pairs, max);
		if (v.count !== v.pairs.length)
			throw fail('InputSchema', 'Prepared pair count differs');
	}
}
export const facts = (v) => ({
	count: v.count,
	requiredSha256: v.requiredSha256,
	resolutionSha256: v.resolutionSha256,
});
const baseKeys = [
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
export function validatePlan5(plan, legacy) {
	shape(
		plan,
		[
			...baseKeys,
			'protocol',
			'registryContract',
			'workspaceBytes',
			'workLimit',
			'sourceRequirements',
		],
		['sourceSha256'],
	);
	if (plan.protocol !== 5 || plan.registryContract !== 1)
		throw fail('InputSchema', 'Prepared source contract');
	if (
		!Number.isSafeInteger(plan.workspaceBytes) ||
		plan.workspaceBytes < 0 ||
		plan.workspaceBytes > 64 * 1024 * 1024 ||
		!Number.isSafeInteger(plan.workLimit) ||
		plan.workLimit < 0 ||
		plan.workLimit > 268435456
	)
		throw fail('ResourceLimit', 'Prepared source limits');
	validateFacts(plan.sourceRequirements, 256, true);
	const base = Object.fromEntries(
		[...baseKeys, 'sourceSha256']
			.filter((k) => Object.hasOwn(plan, k))
			.map((k) => [k, plan[k]]),
	);
	legacy(base);
	encodeProgram(plan, OPEN_BYTES_MAX);
}
export function validateBinding5(binding, plan, legacy) {
	const source = Object.fromEntries(
		Object.entries(binding).filter(
			([key]) =>
				!['candidateRequirements', 'unionRequirements'].includes(key),
		),
	);
	validatePlan5(source, legacy);
	for (const key of ['candidateRequirements', 'unionRequirements']) {
		validateFacts(binding[key], 256);
		if (!sameProgram(binding[key], facts(plan.sourceRequirements)))
			throw fail('CodecFailed', 'Prepared READY requirement binding');
	}
	if (!sameProgram(source, plan))
		throw fail('CodecFailed', 'Prepared source binding mismatch');
}
export function validateRequest5(options, plan, legacy, canonical) {
	shape(options, [
		'generation',
		'operation',
		'target',
		'view',
		'render',
		'candidateRequirements',
		'unionRequirements',
	]);
	validateFacts(options.candidateRequirements, 256);
	validateFacts(options.unionRequirements, 257);
	if (options.operation?.op?.startsWith('filter.mask.'))
		throw fail(
			'CapabilityUnsupported',
			'Mask edits require admitted editor publication',
		);
	const operation = canonical(options.operation);
	if (encoder.encode(operation).length > 4096)
		throw fail('ResourceLimit', 'Prepared operation exceeds4096bytes');
	const old = Object.fromEntries(
		['generation', 'operation', 'target', 'view', 'render'].map((k) => [
			k,
			options[k],
		]),
	);
	legacy(old, { ...plan, protocol: 4 });
	if (options.operation === null) {
		if (
			options.target !== null ||
			!sameProgram(
				options.candidateRequirements,
				facts(plan.sourceRequirements),
			) ||
			!sameProgram(
				options.unionRequirements,
				facts(plan.sourceRequirements),
			)
		)
			throw fail(
				'CodecFailed',
				'Prepared BASE requirements differ from source',
			);
	}
	encodeProgram(options, BEGIN_BYTES_MAX);
}
export function validateResult5(result, request, plan, legacy) {
	for (const key of [
		'sourceRequirements',
		'candidateRequirements',
		'unionRequirements',
	])
		validateFacts(result[key], key === 'unionRequirements' ? 257 : 256);
	if (
		!sameProgram(
			result.sourceRequirements,
			facts(plan.sourceRequirements),
		) ||
		!sameProgram(
			result.candidateRequirements,
			request.candidateRequirements,
		) ||
		!sameProgram(result.unionRequirements, request.unionRequirements)
	)
		throw fail('CodecFailed', 'Prepared result requirements mismatch');
	const old = Object.fromEntries(
		Object.entries(result).filter(
			([key]) =>
				![
					'sourceRequirements',
					'candidateRequirements',
					'unionRequirements',
				].includes(key),
		),
	);
	legacy(old, request, { ...plan, protocol: 4 });
}
export function unionPairs(source, operation) {
	validatePairs(source, 256);
	const pairs = source.map((p) => ({ ...p }));
	if (['filter.add', 'filter.update'].includes(operation?.op)) {
		const p = {
			profile_id: operation?.descriptor?.kind,
			codec_id: 'params-v1',
		};
		id(p.profile_id, 96);
		if (!pairs.some((x) => compare(x, p) === 0)) pairs.push(p);
	}
	pairs.sort(compare);
	validatePairs(pairs);
	return pairs;
}
async function hash(bytes) {
	return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
		.map((x) => x.toString(16).padStart(2, '0'))
		.join('');
}
function bytes(domain, pairs, entries) {
	const out = [...encoder.encode(domain)];
	const u16 = (n) => out.push(n >>> 8, n & 255);
	const text = (s) => {
		const b = encoder.encode(s);
		u16(b.length);
		out.push(...b);
	};
	if (entries) u16(1);
	u16(pairs.length);
	for (let i = 0; i < pairs.length; i++) {
		text(pairs[i].profile_id);
		text(pairs[i].codec_id);
		if (entries) {
			const e = entries[i];
			text(e.resource_contract_id);
			const n = e.resource_contract_revision;
			out.push(n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255);
		}
	}
	return Uint8Array.from(out);
}
export async function requirementDigests(pairs, entries) {
	validatePairs(pairs);
	if (!Array.isArray(entries) || entries.length !== pairs.length)
		throw fail('CodecFailed', 'Missing prepared resolution entry');
	for (let i = 0; i < entries.length; i++) {
		const e = entries[i];
		shape(e, [
			'profile_id',
			'codec_id',
			'resource_contract_id',
			'resource_contract_revision',
		]);
		id(e.profile_id, 96);
		id(e.codec_id, 64);
		id(e.resource_contract_id, 96);
		if (
			compare(e, pairs[i]) !== 0 ||
			!Number.isInteger(e.resource_contract_revision) ||
			e.resource_contract_revision < 1 ||
			e.resource_contract_revision > 0xffffffff
		)
			throw fail('CodecFailed', 'Prepared resolution identity');
	}
	return {
		count: pairs.length,
		requiredSha256: await hash(
			bytes('paintcraft.worker-required/v1\0', pairs),
		),
		resolutionSha256: await hash(
			bytes('paintcraft.worker-resolution/v1\0', pairs, entries),
		),
	};
}
export async function validateCapabilities(reply, pairs, expected) {
	shape(reply, [
		'kind',
		'id',
		'protocol',
		'registryContract',
		'entries',
		'requiredSha256',
		'resolutionSha256',
	]);
	if (
		reply.kind !== 'capabilities' ||
		!Number.isInteger(reply.id) ||
		reply.id < 1 ||
		reply.id > 0xffffffff ||
		reply.protocol !== 5 ||
		reply.registryContract !== 1
	)
		throw fail('CodecFailed', 'Prepared capability handshake');
	const actual = await requirementDigests(pairs, reply.entries);
	encodeProgram(reply, CAPABILITY_BYTES_MAX);
	if (
		reply.requiredSha256 !== actual.requiredSha256 ||
		reply.resolutionSha256 !== actual.resolutionSha256 ||
		!sameProgram(actual, facts(expected))
	)
		throw fail('CodecFailed', 'Prepared capability subset differs');
	return reply.entries.map((e) => Object.freeze({ ...e }));
}
export function validateReady5(ready, plan) {
	shape(ready, ['kind', 'id', 'binding']);
	for (const key of [
		'sourceRequirements',
		'candidateRequirements',
		'unionRequirements',
	]) {
		const value =
			key === 'sourceRequirements'
				? facts(ready.binding[key])
				: ready.binding[key];
		validateFacts(value, 256);
		if (!sameProgram(value, facts(plan.sourceRequirements)))
			throw fail('CodecFailed', 'Prepared READY inventory differs');
	}
}
export function freezeSource5(binding) {
	for (const p of binding.sourceRequirements.pairs) Object.freeze(p);
	Object.freeze(binding.sourceRequirements.pairs);
	Object.freeze(binding.sourceRequirements);
	Object.freeze(binding.candidateRequirements);
	Object.freeze(binding.unionRequirements);
}
