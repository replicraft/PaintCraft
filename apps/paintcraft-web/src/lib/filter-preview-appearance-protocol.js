// Source7 only. Root owns transport slot/cancellation/current-intent lifecycle.
import { encodeProgram } from './filter-preview-program-protocol.js';
import {
	OPEN_BYTES_MAX,
	BEGIN_BYTES_MAX,
	CAPABILITY_BYTES_MAX,
	BASE_WORK_MAX,
	NOISE_WORK_MAX,
	decimal7,
	equal7,
	facts7,
	fail7,
	freeze7,
	mode7,
	numericalFacts7,
	owner7,
	requireMode7,
	sha7,
	shape7,
	snapshot7,
	uint7,
	uuid7,
	validateAdmission7,
	validateAppearance7,
	validateFacts7,
	validatePairs7,
	validateUnion7,
} from './filter-preview-appearance-common.js';
import {
	admissionDigest7,
	numericalDigests7,
	operationDigest7,
	verifyFacts7,
	verifyUnion7,
} from './filter-preview-appearance-framing.js';
import {
	canonicalOperation7,
	geometry7,
	validateOperation7,
} from './filter-preview-appearance-operation.js';
export * from './filter-preview-appearance-common.js';
export * from './filter-preview-appearance-framing.js';
export {
	canonicalOperation7,
	geometry7,
	validateDescriptor7,
	validateOperation7,
} from './filter-preview-appearance-operation.js';
const baseKeys = [
	'owner',
	'draft',
	'document',
	'sourceSha256',
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
function header(value) {
	if (value.protocol !== 7 || value.registryContract !== 3)
		throw fail7('CapabilityUnsupported', 'Source7 contract3');
}
function numerical(value) {
	shape7(value, ['count', 'requiredSha256', 'resolutionSha256']);
	uint7(value.count, 257);
	sha7(value.requiredSha256);
	sha7(value.resolutionSha256);
}
export function validatePreplan7(value) {
	shape7(value, [
		'sourceProtocol',
		'registryContract',
		'mode',
		'owner',
		'draft',
	]);
	if (value.sourceProtocol !== 7 || value.registryContract !== 3)
		throw fail7('CapabilityUnsupported', 'Source7 prelease selector');
	mode7(value.mode);
	owner7(value.owner);
	uuid7(value.draft);
	encodeProgram(value, BEGIN_BYTES_MAX);
}
export function validateRuntimeFacts7(value, mode) {
	mode7(mode);
	shape7(value, [
		'protocol',
		'registryContract',
		'appearanceRevision',
		'queryRevision',
		'coverageAbi',
		'sliceRevision',
		'nativeEdge',
		'baseSliceWork',
		'noiseSliceWork',
		'controlQuoteBytes',
	]);
	header(value);
	if (
		value.appearanceRevision !== 1 ||
		value.queryRevision !== 1 ||
		value.coverageAbi !== 'wasm32-v1' ||
		value.sliceRevision !== 1 ||
		value.nativeEdge !== 64 ||
		value.baseSliceWork !== '8388608' ||
		value.noiseSliceWork !== (mode === 'noise' ? '134217728' : '0')
	)
		throw fail7('CapabilityUnsupported', 'Source7 compiled ABI/slice');
	const bytes = decimal7(value.controlQuoteBytes);
	if (bytes === 0n || bytes > 224n * 1024n * 1024n)
		throw fail7('ResourceLimit', 'Source7 fixed control quote');
}
export function validateReady7(value, mode) {
	shape7(value, ['runtimeFacts', 'controlQuote']);
	validateRuntimeFacts7(value.runtimeFacts, mode);
	decimal7(value.controlQuote);
	if (value.controlQuote !== value.runtimeFacts.controlQuoteBytes)
		throw fail7('CodecFailed', 'Source7 actual control quote equality');
	encodeProgram(value, CAPABILITY_BYTES_MAX);
}
export function validatePlan7(value) {
	shape7(value, [
		...baseKeys,
		'protocol',
		'registryContract',
		'sourceRequirements',
		'appearance',
		'admission',
		'slice',
	]);
	header(value);
	owner7(value.owner);
	uuid7(value.draft);
	uuid7(value.document);
	sha7(value.sourceSha256);
	sha7(value.filterDigest);
	if (!Array.isArray(value.size) || value.size.length !== 2)
		throw fail7('InputSchema', 'Source7 canvas pair');
	for (const n of value.size) uint7(n, 1048576, 1);
	uint7(value.layers, 100000);
	for (const key of ['residentBytes', 'metadataCharge', 'maskDecodeBytes'])
		uint7(value[key], 224 * 1024 * 1024);
	if (value.residentBytes % 262144 !== 0)
		throw fail7('InputSchema', 'Source7 padded resident tiles');
	uint7(value.metadataBytes, 8 * 1024 * 1024);
	if (value.metadataBytes !== 0 && value.metadataCharge === 0)
		throw fail7('InputSchema', 'Source7 metadata reservation');
	uint7(value.maskCount, value.layers * 256);
	uint7(value.maskBands, 262144);
	uint7(value.maskRuns, 262144);
	if (
		value.maskBands > value.maskRuns ||
		(value.maskBands === 0) !== (value.maskRuns === 0) ||
		(value.maskCount === 0 &&
			(value.maskBands || value.maskRuns || value.maskDecodeBytes)) ||
		(value.maskBands === 0) !== (value.maskDecodeBytes === 0)
	)
		throw fail7(
			'InputSchema',
			'Source7 FILTER-only canonical coverage facts',
		);
	validateAppearance7(value.appearance);
	validateAdmission7(value.admission, value.slice);
	validateFacts7(value.sourceRequirements, true);
	requireMode7(value.sourceRequirements.pairs, value.admission);
	const own = value.sourceRequirements.layerMasks;
	if (
		own.stored < 1 ||
		own.stored > value.layers ||
		own.bands + value.maskBands > 262144 ||
		own.runs + value.maskRuns > 262144
	)
		throw fail7('ResourceLimit', 'Source7 combined own/filter counts');
	const minimum =
		BigInt(value.residentBytes) +
		BigInt(value.metadataCharge) +
		BigInt(value.maskDecodeBytes) +
		decimal7(own.decodeUpperBytes) +
		decimal7(value.admission.workspaceBytes);
	if (minimum > 224n * 1024n * 1024n)
		throw fail7('ResourceLimit', 'Source7 declared payload lower bound');
	// Rust independently prices actual ABI/control/index/retained overlap; this
	// declaration check is never a substitute for its same-witness admission.
	encodeProgram(value, OPEN_BYTES_MAX);
}
export async function verifyPlan7(value, entryValue) {
	const plan = snapshot7(value, OPEN_BYTES_MAX),
		entries = snapshot7(entryValue);
	validatePlan7(plan);
	const actual = await numericalDigests7(
		plan.sourceRequirements.pairs,
		entries,
	);
	if (!equal7(actual, numericalFacts7(plan.sourceRequirements)))
		throw fail7('CodecFailed', 'Source7 resolved source subset');
	await verifyFacts7(facts7(plan.sourceRequirements), plan.admission);
	return freeze7(plan);
}
export function validateHello7(value) {
	shape7(value, [
		'protocol',
		'registryContract',
		'pairs',
		'appearance',
		'admission',
		'slice',
	]);
	header(value);
	validatePairs7(value.pairs);
	validateAppearance7(value.appearance);
	validateAdmission7(value.admission, value.slice);
	requireMode7(value.pairs, value.admission);
	encodeProgram(value, CAPABILITY_BYTES_MAX);
}
export async function validateCapabilities7(
	value,
	pairValue,
	expectedValue,
	admissionValue,
	runtimeValue,
) {
	// Every caller-owned input is detached synchronously before the first await.
	const reply = snapshot7(value),
		pairs = snapshot7(pairValue),
		expected = snapshot7(expectedValue),
		admission = snapshot7(admissionValue),
		runtime = snapshot7(runtimeValue);
	shape7(reply, [
		'kind',
		'protocol',
		'registryContract',
		'entries',
		'appearance',
		'admissionSha256',
		'requiredSha256',
		'resolutionSha256',
		'runtimeFacts',
	]);
	header(reply);
	if (reply.kind !== 'capabilities')
		throw fail7('CodecFailed', 'Source7 HELLO kind');
	validatePairs7(pairs);
	numerical(expected);
	validateAdmission7(admission);
	requireMode7(pairs, admission);
	validateAppearance7(reply.appearance);
	sha7(reply.admissionSha256);
	sha7(reply.requiredSha256);
	sha7(reply.resolutionSha256);
	validateRuntimeFacts7(runtime, admission.mode);
	validateRuntimeFacts7(reply.runtimeFacts, admission.mode);
	if (!equal7(reply.runtimeFacts, runtime))
		throw fail7('CodecFailed', 'Source7 actual getter/HELLO facts');
	const actual = await numericalDigests7(pairs, reply.entries);
	if (
		!equal7(actual, expected) ||
		reply.requiredSha256 !== actual.requiredSha256 ||
		reply.resolutionSha256 !== actual.resolutionSha256 ||
		reply.admissionSha256 !== (await admissionDigest7(admission))
	)
		throw fail7('CodecFailed', 'Source7 requested HELLO subset binding');
	return freeze7(reply.entries);
}
export async function validateCapabilitiesMessage7(
	value,
	pairs,
	expected,
	admission,
	runtime,
) {
	const message = snapshot7(value);
	uint7(message.id, 0xffffffff, 1);
	const metadata = { ...message };
	delete metadata.id;
	return validateCapabilities7(metadata, pairs, expected, admission, runtime);
}
export function validateBegin7(value, plan) {
	validatePlan7(plan);
	shape7(value, [
		'protocol',
		'registryContract',
		'owner',
		'draft',
		'sourceSha256',
		'generation',
		'operation',
		'target',
		'view',
		'render',
		'candidateRequirements',
		'unionRequirements',
		'admissionSha256',
	]);
	header(value);
	owner7(value.owner);
	uuid7(value.draft);
	sha7(value.sourceSha256);
	decimal7(value.generation);
	sha7(value.admissionSha256);
	if (typeof value.render !== 'boolean')
		throw fail7('InputSchema', 'Source7 render bool');
	validateOperation7(value.operation, value.target);
	canonicalOperation7(value.operation);
	geometry7(value.view, plan.size);
	if (
		plan.admission.mode === 'base' &&
		value.operation?.descriptor?.kind === 'add-noise-rgb-v1'
	)
		throw fail7(
			'CapabilityUnsupported',
			'New typed Noise intent requires closeACK/new OPEN',
		);
	validateFacts7(value.candidateRequirements);
	validateUnion7(value.unionRequirements);
	for (const key of ['owner', 'draft', 'sourceSha256'])
		if (!equal7(value[key], plan[key]))
			throw fail7('ConflictStaleBase', 'Source7 immutable owner/source');
	if (
		value.candidateRequirements.sourceCoverageSha256 !==
			plan.sourceRequirements.sourceCoverageSha256 ||
		value.unionRequirements.sourceCoverageSha256 !==
			plan.sourceRequirements.sourceCoverageSha256 ||
		!equal7(
			value.candidateRequirements.layerMasks,
			plan.sourceRequirements.layerMasks,
		)
	)
		throw fail7('ConflictStaleBase', 'Source7 immutable wire/own anchors');
	if (
		value.operation === null &&
		!equal7(value.candidateRequirements, facts7(plan.sourceRequirements))
	)
		throw fail7('CodecFailed', 'Source7 BASE candidate facts');
	encodeProgram(value, BEGIN_BYTES_MAX);
}
export async function verifyBegin7(value, planValue) {
	const request = snapshot7(value, BEGIN_BYTES_MAX),
		plan = snapshot7(planValue, OPEN_BYTES_MAX);
	validateBegin7(request, plan);
	if (request.admissionSha256 !== (await admissionDigest7(plan.admission)))
		throw fail7('ConflictStaleBase', 'Source7 sealed admission');
	await verifyFacts7(request.candidateRequirements, plan.admission);
	await verifyUnion7(request.unionRequirements, plan.admission);
	return freeze7(request);
}
export function validateWorkQuote7(value, admission) {
	shape7(value, [
		'baseWork',
		'noiseWork',
		'totalWork',
		'maskWorkActual',
		'maskWorkUpper',
		'maskWorkMargin',
	]);
	validateAdmission7(admission);
	const base = decimal7(value.baseWork),
		noise = decimal7(value.noiseWork),
		total = decimal7(value.totalWork),
		actual = decimal7(value.maskWorkActual),
		upper = decimal7(value.maskWorkUpper),
		margin = decimal7(value.maskWorkMargin);
	if (
		base > decimal7(admission.baseWorkLimit) ||
		noise > decimal7(admission.noiseWorkLimit) ||
		base + noise !== total ||
		actual + margin !== upper ||
		upper > base ||
		total > BASE_WORK_MAX + NOISE_WORK_MAX
	)
		throw fail7('CodecFailed', 'Source7 split work/mask margin');
}
export function validateBinding7(value, request, plan) {
	validateBegin7(request, plan);
	shape7(value, [
		'kind',
		'protocol',
		'registryContract',
		'generation',
		'owner',
		'draft',
		'sourceSha256',
		'target',
		'view',
		'width',
		'height',
		'region',
		'level',
		'operationSha256',
		'admissionSha256',
		'sourceRequirements',
		'candidateRequirements',
		'unionRequirements',
		'workQuote',
	]);
	header(value);
	owner7(value.owner);
	uuid7(value.draft);
	sha7(value.sourceSha256);
	decimal7(value.generation);
	sha7(value.operationSha256);
	sha7(value.admissionSha256);
	if (value.kind !== (request.render ? 'frame' : 'prepared'))
		throw fail7('CodecFailed', 'Source7 prepared/frame kind');
	for (const key of [
		'owner',
		'draft',
		'sourceSha256',
		'generation',
		'target',
		'view',
		'admissionSha256',
		'candidateRequirements',
		'unionRequirements',
	])
		if (!equal7(value[key], request[key]))
			throw fail7('CodecFailed', 'Source7 exact result binding');
	validateFacts7(value.sourceRequirements);
	if (!equal7(value.sourceRequirements, facts7(plan.sourceRequirements)))
		throw fail7('CodecFailed', 'Source7 source result fact');
	const geometry = geometry7(request.view, plan.size);
	for (const key of ['region', 'width', 'height', 'level'])
		if (!equal7(value[key], geometry[key]))
			throw fail7('CodecFailed', 'Source7 quantized result geometry');
	validateWorkQuote7(value.workQuote, plan.admission);
	encodeProgram(value, CAPABILITY_BYTES_MAX);
}
export async function verifyBinding7(value, requestValue, planValue) {
	const binding = snapshot7(value),
		request = snapshot7(requestValue, BEGIN_BYTES_MAX),
		plan = snapshot7(planValue, OPEN_BYTES_MAX);
	validateBinding7(binding, request, plan);
	if (binding.operationSha256 !== (await operationDigest7(request.operation)))
		throw fail7('CodecFailed', 'Source7 operation SHA');
	await verifyBegin7(request, plan);
	return freeze7(binding);
}
export function validateContext7(value, plan, request) {
	if (request === undefined) {
		validatePlan7(value);
		if (!equal7(value, plan))
			throw fail7('CodecFailed', 'Source7 original OPEN context');
	} else validateBinding7(value, request, plan);
}
export function validateCopyAdmitted7(
	value,
	request,
	plan,
	expectedOutputSha256,
) {
	shape7(value, ['kind', 'outputVersion', 'outputSha256', 'binding']);
	if (
		request.render !== true ||
		request.operation !== null ||
		request.view?.level !== 0
	)
		throw fail7(
			'CapabilityUnsupported',
			'Source7 visual copy requires level0 BASE frame',
		);
	if (value.kind !== 'copy-admitted' || value.outputVersion !== 1)
		throw fail7('CapabilityUnsupported', 'Source7 visual copy admission');
	sha7(value.outputSha256);
	sha7(expectedOutputSha256);
	if (value.outputSha256 !== expectedOutputSha256)
		throw fail7('CodecFailed', 'Source7 frozen output intent');
	validateBinding7(value.binding, request, plan);
}
