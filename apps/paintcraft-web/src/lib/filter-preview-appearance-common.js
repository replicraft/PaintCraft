// Source7 metadata only. These facts never create execution or byte authority.
import {
	encodeProgram,
	validatePairs,
} from './filter-preview-program-protocol.js';
export const OPEN_BYTES_MAX = 64 * 1024;
export const CAPABILITY_BYTES_MAX = 128 * 1024;
export const BEGIN_BYTES_MAX = 8 * 1024;
export const U64_MAX = 0xffffffffffffffffn;
export const BASE_WORK_MAX = 268435456n;
export const NOISE_WORK_MAX = 34359738368n;
export const APPEARANCE7 = Object.freeze({
	feature_id: 'layer-masks.v1',
	resource_contract_id: 'layer-mask-final-alpha-rgba8-v1',
	resource_contract_revision: 2,
	query_revision: 1,
	alpha_revision: 1,
	workload_class_id: 'base-work-v1',
	workload_class_revision: 1,
});
export const fail7 = (code, message) =>
	Object.assign(new Error(message), { code });
export function shape7(value, keys, optional = []) {
	if (
		!value ||
		typeof value !== 'object' ||
		Array.isArray(value) ||
		Object.getPrototypeOf(value) !== Object.prototype
	)
		throw fail7('InputSchema', 'Source7 plain object');
	const names = Object.keys(value);
	if (
		names.length > keys.length + optional.length ||
		names.some((key) => !keys.includes(key) && !optional.includes(key)) ||
		keys.some((key) => !Object.hasOwn(value, key))
	)
		throw fail7('InputSchema', 'Source7 closed fields');
}
export function uint7(value, max, min = 0) {
	if (!Number.isSafeInteger(value) || value < min || value > max)
		throw fail7('InputSchema', 'Source7 integer bound');
	return value;
}
export function decimal7(value) {
	if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value))
		throw fail7('InputSchema', 'Source7 canonical u64');
	const n = BigInt(value);
	if (n > U64_MAX) throw fail7('InputSchema', 'Source7 u64 overflow');
	return n;
}
export function sha7(value) {
	if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value))
		throw fail7('InputSchema', 'Source7 lowercase SHA256');
	return value;
}
export function uuid7(value) {
	if (
		typeof value !== 'string' ||
		!/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(value)
	)
		throw fail7('InputSchema', 'Source7 canonical UUID');
}
export function owner7(value) {
	shape7(value, ['session', 'state', 'epoch', 'frontend']);
	uuid7(value.session);
	uuid7(value.state);
	decimal7(value.epoch);
	if (value.frontend !== 3) throw fail7('InputSchema', 'Source7 frontend');
}
export function snapshot7(value, max = CAPABILITY_BYTES_MAX) {
	// The existing incremental serializer checks every emitted token before any
	// aggregate buffer; hostile toJSON is never invoked. Detach BEFORE awaits.
	return JSON.parse(encodeProgram(value, max));
}
export function equal7(a, b) {
	return (
		encodeProgram(a, CAPABILITY_BYTES_MAX) ===
		encodeProgram(b, CAPABILITY_BYTES_MAX)
	);
}
export function freeze7(value) {
	if (value && typeof value === 'object') {
		for (const child of Object.values(value)) freeze7(child);
		Object.freeze(value);
	}
	return value;
}
export function validateAppearance7(value) {
	shape7(value, Object.keys(APPEARANCE7));
	if (!equal7(value, APPEARANCE7))
		throw fail7('CapabilityUnsupported', 'Source7 appearance registration');
}
export function mode7(value) {
	if (value !== 'base' && value !== 'noise')
		throw fail7('InputSchema', 'Source7 mode');
	return value;
}
export function slice7(mode) {
	mode7(mode);
	return {
		revision: 1,
		nativeEdge: 64,
		baseWorkLimit: '8388608',
		noiseWorkLimit: mode === 'noise' ? '134217728' : '0',
	};
}
export function validateAdmission7(value, slice = slice7(value?.mode)) {
	shape7(value, [
		'mode',
		'workAuthority',
		'workspaceBytes',
		'baseWorkLimit',
		'noiseWorkLimit',
		'totalWorkLimit',
	]);
	mode7(value.mode);
	if (!['captured-job', 'standalone-view-jobs'].includes(value.workAuthority))
		throw fail7('InputSchema', 'Source7 explicit work authority');
	const bytes = decimal7(value.workspaceBytes),
		base = decimal7(value.baseWorkLimit),
		noise = decimal7(value.noiseWorkLimit),
		total = decimal7(value.totalWorkLimit);
	if (
		bytes > 67108864n ||
		base > BASE_WORK_MAX ||
		noise > NOISE_WORK_MAX ||
		(value.mode === 'base' && noise !== 0n)
	)
		throw fail7('ResourceLimit', 'Source7 admission limit');
	if (base + noise !== total || total > U64_MAX)
		throw fail7('InputSchema', 'Source7 total work');
	shape7(slice, [
		'revision',
		'nativeEdge',
		'baseWorkLimit',
		'noiseWorkLimit',
	]);
	if (!equal7(slice, slice7(value.mode)))
		throw fail7('CapabilityUnsupported', 'Source7 fixed slice policy');
}
export function validateLayerMasks7(value) {
	shape7(value, [
		'stored',
		'enabled',
		'bands',
		'runs',
		'decodeUpperBytes',
		'identitySha256',
		'coverageQuoteRevision',
		'coverageAbi',
	]);
	uint7(value.stored, 100000);
	uint7(value.enabled, value.stored);
	uint7(value.bands, 262144);
	uint7(value.runs, 262144);
	if (value.bands > value.runs || (value.bands === 0) !== (value.runs === 0))
		throw fail7('InputSchema', 'Source7 own canonical counts');
	decimal7(value.decodeUpperBytes);
	sha7(value.identitySha256);
	if (value.coverageQuoteRevision !== 1 || value.coverageAbi !== 'wasm32-v1')
		throw fail7(
			'CapabilityUnsupported',
			'Source7 compiled browser coverage ABI',
		);
}
export function validateFacts7(value, full = false) {
	shape7(value, [
		'count',
		'requiredSha256',
		'resolutionSha256',
		'layerMasks',
		'sourceCoverageSha256',
		'maskInventorySha256',
		'capabilitySha256',
		...(full ? ['pairs'] : []),
	]);
	uint7(value.count, 256);
	validateLayerMasks7(value.layerMasks);
	for (const key of [
		'requiredSha256',
		'resolutionSha256',
		'sourceCoverageSha256',
		'maskInventorySha256',
		'capabilitySha256',
	])
		sha7(value[key]);
	if (full) {
		validatePairs7(value.pairs, 256);
		if (value.count !== value.pairs.length)
			throw fail7('InputSchema', 'Source7 source pair count');
	}
}
export function facts7(value) {
	validateFacts7(value, Object.hasOwn(value, 'pairs'));
	const copy = { ...value, layerMasks: { ...value.layerMasks } };
	delete copy.pairs;
	return copy;
}
export function validateUnion7(value) {
	shape7(value, [
		'count',
		'requiredSha256',
		'resolutionSha256',
		'sourceCoverageSha256',
		'capabilitySha256',
	]);
	uint7(value.count, 257);
	for (const key of [
		'requiredSha256',
		'resolutionSha256',
		'sourceCoverageSha256',
		'capabilitySha256',
	])
		sha7(value[key]);
}
export function numericalFacts7(value) {
	return {
		count: value.count,
		requiredSha256: value.requiredSha256,
		resolutionSha256: value.resolutionSha256,
	};
}
const resources = Object.freeze({
	'invert-rgb-v1': 'direct-rgba8-point-v1',
	'brightness-contrast-v1': 'direct-rgba8-point-v1',
	'black-white-rgb-v1': 'direct-rgba8-point-v1',
	'threshold-luma-v1': 'direct-rgba8-point-v1',
	'posterize-rgb-v1': 'direct-rgba8-point-v1',
	'solarize-rgb-v1': 'direct-rgba8-point-v1',
	'box-blur-v1': 'premultiplied-region-rgba8-v1',
	'levels-rgb-v1': 'prepared-levels-rgba8-v1',
	'exposure-srgb-v1': 'prepared-exposure-rgba8-v1',
	'curves-rgb-v1': 'prepared-curves-rgba8-v1',
	'vibrance-rgb-v1': 'direct-vibrance-rgba8-work64-v1',
	'hue-saturation-rgb-v1': 'direct-hue-saturation-rgba8-work128-v1',
	'add-noise-rgb-v1': 'direct-source-noise-rgba8-u512-g4096-v1',
});
export function numericalEntry7(pair) {
	if (
		!Object.hasOwn(resources, pair.profile_id) ||
		pair.codec_id !== 'params-v1'
	)
		throw fail7(
			'CapabilityUnsupported',
			'Source7 unknown numerical registration',
		);
	return {
		...pair,
		resource_contract_id: resources[pair.profile_id],
		resource_contract_revision: 1,
		workload_class_id:
			pair.profile_id === 'add-noise-rgb-v1'
				? 'noise-source-work-v1'
				: 'base-work-v1',
		workload_class_revision: 1,
	};
}
export function validatePairs7(pairs, max = 257) {
	validatePairs(pairs, max);
	for (const pair of pairs) numericalEntry7(pair);
}
export function requireMode7(pairs, admission) {
	if (
		admission.mode === 'base' &&
		pairs.some((p) => p.profile_id === 'add-noise-rgb-v1')
	)
		throw fail7(
			'CapabilityUnsupported',
			'Stored or prospective Noise requires a new Noise OPEN',
		);
}
