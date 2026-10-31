export const SOURCE_BYTES_MAX = 64 * 1024 * 1024;
export const OUTPUT_PIXELS_MAX = 4_194_304;
export const LINEAR_MEMORY_BYTES = 256 * 1024 * 1024;
export const SPATIAL_WORKSPACE_BYTES_MAX = 64 * 1024 * 1024;
export const SPATIAL_JOB_WORK_MAX = 268435456;
export const SPATIAL_STEP_WORK_MAX = 8388608;
// One persistent instance plus serialization/bridge and bounded upload slots.
// A logical declaration; neither allocator interception nor GPU/RSS accounting.
export const RESERVATION_BYTES =
	LINEAR_MEMORY_BYTES + 2 * SOURCE_BYTES_MAX + 4 * OUTPUT_PIXELS_MAX * 4;
export function fail(code, message) {
	return Object.assign(new Error(message), { code });
}
export function canonical(value) {
	let nodes = 0,
		characters = 0;
	function visit(v, depth) {
		if (++nodes > 256 || depth > 8)
			throw fail('InputSchema', 'Filter control nesting/extent');
		if (
			v === null ||
			typeof v === 'boolean' ||
			(typeof v === 'number' && Number.isSafeInteger(v))
		)
			return JSON.stringify(v);
		if (typeof v === 'string') {
			characters += v.length;
			if (characters > 4096)
				throw fail('InputSchema', 'Filter control string extent');
			return JSON.stringify(v);
		}
		if (Array.isArray(v)) {
			if (v.length > 256)
				throw fail('InputSchema', 'Filter array extent');
			return `[${v.map((x) => visit(x, depth + 1)).join(',')}]`;
		}
		if (
			typeof v !== 'object' ||
			Object.getPrototypeOf(v) !== Object.prototype
		)
			throw fail('InputSchema', 'Filter control object');
		const keys = [];
		for (const key in v) {
			if (keys.length >= 32 || key.length > 64)
				throw fail('InputSchema', 'Filter object extent');
			keys.push(key);
		}
		return `{${keys
			.sort()
			.map((k) => `${JSON.stringify(k)}:${visit(v[k], depth + 1)}`)
			.join(',')}}`;
	}
	const encoded = visit(value, 0);
	if (encoded.length > 4096)
		throw fail('InputSchema', 'Filter control extent');
	return encoded;
}
function shape(value, keys, optional = []) {
	if (
		!value ||
		typeof value !== 'object' ||
		Array.isArray(value) ||
		Object.getPrototypeOf(value) !== Object.prototype
	)
		throw fail('InputSchema', 'Filter object shape');
	let count = 0;
	for (const key in value) {
		if (
			++count > keys.length + optional.length ||
			(!keys.includes(key) && !optional.includes(key))
		)
			throw fail('InputSchema', 'Unknown filter control fields');
	}
	if (keys.some((k) => !Object.hasOwn(value, k)))
		throw fail('InputSchema', 'Missing filter control fields');
}
function owner(value) {
	shape(value, ['session', 'state', 'epoch', 'frontend']);
	if (
		!isUuid(value.session) ||
		!isUuid(value.state) ||
		value.frontend !== 3 ||
		typeof value.epoch !== 'string' ||
		!/^(0|[1-9]\d{0,19})$/.test(value.epoch) ||
		BigInt(value.epoch) > 0xffffffffffffffffn
	)
		throw fail('InputSchema', 'Filter owner');
}
function isUuid(x) {
	return (
		typeof x === 'string' &&
		/^[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12}$/i.test(x)
	);
}
function isDigest(x) {
	return typeof x === 'string' && /^[\da-f]{64}$/.test(x);
}
function view(value, plan) {
	shape(value, ['region', 'level']);
	if (
		!Array.isArray(value.region) ||
		value.region.length !== 4 ||
		value.region.some((n) => !Number.isSafeInteger(n)) ||
		!Number.isInteger(value.level) ||
		value.level < 0 ||
		value.level > 20
	)
		throw fail('InputSchema', 'Filter viewport');
	const [x, y, w, h] = value.region;
	if (
		x < 0 ||
		y < 0 ||
		w < 1 ||
		h < 1 ||
		x + w > plan.size[0] ||
		y + h > plan.size[1]
	)
		throw fail('InputSchema', 'Filter viewport bounds');
}
export async function sha(bytes) {
	return Array.from(
		new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)),
		(b) => b.toString(16).padStart(2, '0'),
	).join('');
}
export function same(a, b) {
	return canonical(a) === canonical(b);
}
export function validatePlan(plan) {
	shape(
		plan,
		[
			...(Object.hasOwn(plan ?? {}, 'protocol')
				? ['protocol', 'spatialCount', 'workspaceBytes', 'workLimit']
				: []),
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
		],
		['sourceSha256'],
	);
	owner(plan.owner);
	const integer = (x) => Number.isSafeInteger(x) && x >= 0;
	if (Object.hasOwn(plan, 'protocol')) {
		if (
			plan.protocol !== 3 ||
			!integer(plan.spatialCount) ||
			!integer(plan.workspaceBytes) ||
			!integer(plan.workLimit)
		)
			throw fail(
				'InputSchema',
				'Invalid spatial source protocol/workspace',
			);
		if (
			plan.spatialCount > plan.layers * 256 ||
			plan.workspaceBytes > SPATIAL_WORKSPACE_BYTES_MAX ||
			plan.workLimit > SPATIAL_JOB_WORK_MAX
		)
			throw fail(
				'ResourceLimit',
				'Spatial source workspace declaration exceeds profile',
			);
	}

	if (
		!isUuid(plan.draft) ||
		!isUuid(plan.document) ||
		!Array.isArray(plan.size) ||
		plan.size.length !== 2 ||
		plan.size.some((n) => !integer(n) || n < 1 || n > 1_048_576) ||
		!integer(plan.layers) ||
		plan.layers > 100_000 ||
		!integer(plan.residentBytes) ||
		plan.residentBytes % (256 * 256 * 4) !== 0 ||
		!integer(plan.metadataBytes) ||
		plan.metadataBytes > 8 * 1024 * 1024 ||
		!integer(plan.metadataCharge) ||
		!integer(plan.maskCount) ||
		plan.maskCount > plan.layers * 256 ||
		!integer(plan.maskBands) ||
		plan.maskBands > 262144 ||
		!integer(plan.maskRuns) ||
		plan.maskRuns > 262144 ||
		!integer(plan.maskDecodeBytes) ||
		plan.maskDecodeBytes > LINEAR_MEMORY_BYTES ||
		plan.maskBands > plan.maskRuns ||
		(plan.maskBands === 0) !== (plan.maskRuns === 0) ||
		(plan.maskCount === 0 &&
			(plan.maskBands !== 0 ||
				plan.maskRuns !== 0 ||
				plan.maskDecodeBytes !== 0)) ||
		(plan.maskBands === 0 && plan.maskDecodeBytes !== 0) ||
		(plan.maskBands > 0 && plan.maskDecodeBytes === 0) ||
		plan.residentBytes + plan.metadataCharge + plan.maskDecodeBytes >
			LINEAR_MEMORY_BYTES ||
		!isDigest(plan.filterDigest) ||
		(plan.sourceSha256 !== undefined && !isDigest(plan.sourceSha256))
	)
		throw fail('InputSchema', 'Invalid filter preview source facts');
}
export function validateBinding(binding, plan) {
	validatePlan(binding);
	if ((binding.protocol ?? 2) !== (plan.protocol ?? 2))
		throw fail('CodecFailed', 'Render Worker source profile mismatch');
	for (const key of [
		...(plan.protocol === 3
			? [
					'protocol',
					'spatialCount',
					'workspaceBytes',
					'workLimit',
					'layers',
					'residentBytes',
					'metadataBytes',
					'metadataCharge',
				]
			: []),
		'owner',
		'draft',
		'document',
		'sourceSha256',
		'size',
		'filterDigest',
		'maskCount',
		'maskBands',
		'maskRuns',
		'maskDecodeBytes',
	])
		if (!same(binding?.[key], plan[key]))
			throw fail('CodecFailed', 'Render Worker source binding mismatch');
}
export function validateRequest(options, plan) {
	shape(options, ['generation', 'operation', 'target', 'view', 'render']);
	if (
		plan.protocol !== 3 &&
		options.operation?.descriptor?.kind === 'box-blur-v1'
	)
		throw fail(
			'CapabilityUnsupported',
			'Box preview requires source protocol3',
		);
	view(options.view, plan);
	if (
		typeof options.generation !== 'string' ||
		!/^(0|[1-9]\d{0,19})$/.test(options.generation) ||
		BigInt(options.generation) > 0xffffffffffffffffn ||
		typeof options.render !== 'boolean' ||
		(options.target !== null && !isUuid(options.target))
	)
		throw fail('InputSchema', 'Invalid filter request');
	canonical(options.operation);
}
export function validateResult(result, request, plan) {
	shape(
		result,
		[
			'kind',
			'id',
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
		],
		['pixels'],
	);
	owner(result.owner);
	view(result.view, plan);
	if (
		typeof result.generation !== 'string' ||
		!/^(0|[1-9]\d{0,19})$/.test(result.generation) ||
		!isUuid(result.draft) ||
		!isDigest(result.sourceSha256) ||
		!isDigest(result.operationSha256) ||
		(result.target !== null && !isUuid(result.target))
	)
		throw fail('InputSchema', 'Invalid result identity');
	for (const key of ['owner', 'draft', 'sourceSha256'])
		if (!same(result?.[key], plan[key]))
			throw fail('CodecFailed', 'Render Worker owner binding mismatch');
	for (const key of ['generation', 'target', 'view', 'operationSha256'])
		if (!same(result?.[key], request[key]))
			throw fail('CodecFailed', 'Render Worker result binding mismatch');
	if (
		!['frame', 'prepared'].includes(result.kind) ||
		!Number.isSafeInteger(result.width) ||
		result.width < 1 ||
		!Number.isSafeInteger(result.height) ||
		result.height < 1 ||
		result.width * result.height > OUTPUT_PIXELS_MAX ||
		!Array.isArray(result.region) ||
		result.region.length !== 4 ||
		result.level !== request.view.level
	)
		throw fail('CodecFailed', 'Render Worker output geometry');
	const [x, y, w, h] = request.view.region,
		step = 2 ** request.view.level;
	const expected = [
		Math.floor(x / step) * step,
		Math.floor(y / step) * step,
		Math.min(Math.ceil((x + w) / step) * step, plan.size[0]),
		Math.min(Math.ceil((y + h) / step) * step, plan.size[1]),
	];
	expected[2] -= expected[0];
	expected[3] -= expected[1];
	if (
		!same(result.region, expected) ||
		result.width !== Math.ceil(expected[2] / step) ||
		result.height !== Math.ceil(expected[3] / step)
	)
		throw fail('CodecFailed', 'Render Worker aligned region mismatch');
	if (
		result.kind === 'frame' &&
		(!(result.pixels instanceof Uint8Array) ||
			result.pixels.byteLength !== result.width * result.height * 4)
	)
		throw fail('CodecFailed', 'Render Worker pixel extent');
}
