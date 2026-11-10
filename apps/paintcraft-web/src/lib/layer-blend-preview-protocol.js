// Separate Source8 schemas. No import or activation of the Source4–7 routes.
import {
	bind8,
	buffer8,
	copy8,
	decimal8,
	equal8,
	fail8,
	IDENTITY8,
	identity8,
	LIMITS8,
	pair8,
	ROLES8,
	rect8,
	shape8,
	uint8,
	VERSIONS8,
} from './layer-blend-preview-common.js';

export { LIMITS8, VERSIONS8 } from './layer-blend-preview-common.js';

function header(v, kind) {
	uint8(v.id, 0xffffffff, 1);
	if (v.kind !== kind || v.protocol !== 8)
		fail8('CapabilityUnsupported', 'Protocol8 message selector');
}
function runtime(v) {
	shape8(v, [...Object.keys(VERSIONS8), 'limits', 'controls']);
	for (const [key, n] of Object.entries(VERSIONS8))
		if (v[key] !== n)
			fail8('CapabilityUnsupported', `Protocol8 compiled ${key}`);
	shape8(v.limits, Object.keys(LIMITS8));
	if (!equal8(v.limits, LIMITS8))
		fail8('CapabilityUnsupported', 'Protocol8 compiled ceilings');
	shape8(v.controls, ['serviceBytes', 'stageBytes', 'viewBytes']);
	for (const n of Object.values(v.controls))
		if (decimal8(n, 1n) > BigInt(LIMITS8.jobBytes))
			fail8('ResourceLimit', 'Protocol8 controls');
}
export function validateReady8(v, actualCompiledFacts) {
	shape8(v, ['kind', 'runtimeFacts']);
	if (v.kind !== 'ready') fail8('CapabilityUnsupported', 'Protocol8 READY');
	runtime(v.runtimeFacts);
	runtime(actualCompiledFacts);
	if (!equal8(v.runtimeFacts, actualCompiledFacts))
		fail8(
			'CapabilityUnsupported',
			'Protocol8 actual getter/READY equality',
		);
	return copy8(v);
}
export function validateOpen8(v) {
	shape8(v, [
		'kind',
		'protocol',
		'id',
		...IDENTITY8,
		'byteLength',
		'canvas',
		'storedNonNormal',
		'storedNoise',
		'mode',
		'roles',
		'pairedBytes',
		'baseWork',
		'noiseWork',
	]);
	header(v, 'open');
	identity8(v);
	pair8(v.canvas);
	const n = decimal8(v.byteLength, 1n);
	if (n > BigInt(LIMITS8.encodedBytes))
		fail8('ResourceLimit', 'Protocol8 encoded limit');
	if (
		v.storedNonNormal !== true ||
		typeof v.storedNoise !== 'boolean' ||
		!['base', 'noise'].includes(v.mode) ||
		(v.mode === 'base' && v.storedNoise)
	)
		fail8('CapabilityUnsupported', 'Protocol8 stored source/mode');
	shape8(v.roles, ROLES8);
	let sum = decimal8(v.pairedBytes);
	for (const key of ROLES8) sum += decimal8(v.roles[key]);
	if (sum > BigInt(LIMITS8.jobBytes) || decimal8(v.roles.E) < n)
		fail8('ResourceLimit', 'Protocol8 whole declared job envelope');
	if (
		decimal8(v.baseWork, 1n) > BigInt(LIMITS8.baseWork) ||
		decimal8(v.noiseWork) > BigInt(LIMITS8.noiseWork) ||
		(v.mode === 'base' && v.noiseWork !== '0')
	)
		fail8('ResourceLimit', 'Protocol8 split work ceilings');
	// Demand-only metadata: Rust must still require minima and exact originals.
	return copy8(v);
}
export function validateSourceBuffer8(buffer, open) {
	validateOpen8(open);
	buffer8(buffer, Number(decimal8(open.byteLength)));
}
export function validateOpened8(v, open) {
	validateOpen8(open);
	shape8(v, [
		'kind',
		'protocol',
		'id',
		...IDENTITY8,
		'byteLength',
		'canvas',
		'storedNonNormal',
		'storedNoise',
	]);
	header(v, 'opened');
	identity8(v);
	pair8(v.canvas);
	bind8(v, open, [
		...IDENTITY8,
		'id',
		'byteLength',
		'canvas',
		'storedNonNormal',
		'storedNoise',
	]);
	// This checks the reply binding, not SHA authentication or decoded truth.
}
export function geometry8(visible, level, canvas) {
	pair8(canvas);
	rect8(visible, canvas);
	uint8(level, 20);
	const step = 2 ** level;
	const region = [
		Math.floor(visible[0] / step) * step,
		Math.floor(visible[1] / step) * step,
		Math.min(Math.ceil(visible[2] / step) * step, canvas[0]),
		Math.min(Math.ceil(visible[3] / step) * step, canvas[1]),
	];
	const size = [
		Math.ceil((region[2] - region[0]) / step),
		Math.ceil((region[3] - region[1]) / step),
	];
	if (size[0] * size[1] > LIMITS8.outputCells)
		fail8('ResourceLimit', 'Protocol8 output cells');
	return {
		region,
		size,
		patches: Math.ceil(size[0] / 64) * Math.ceil(size[1] / 64),
	};
}
export function validateView8(v, open) {
	validateOpen8(open);
	shape8(v, [
		'kind',
		'protocol',
		'id',
		...IDENTITY8,
		'token',
		'generation',
		'visible',
		'level',
	]);
	header(v, 'view');
	identity8(v);
	bind8(v, open);
	decimal8(v.token, 1n);
	decimal8(v.generation, 1n);
	geometry8(v.visible, v.level, open.canvas);
	return copy8(v);
}
export function patchRect8(view, canvas, ordinal) {
	const g = geometry8(view.visible, view.level, canvas);
	uint8(ordinal, g.patches - 1);
	const columns = Math.ceil(g.size[0] / 64),
		x = (ordinal % columns) * 64,
		y = Math.floor(ordinal / columns) * 64;
	return [x, y, Math.min(x + 64, g.size[0]), Math.min(y + 64, g.size[1])];
}
const operationKeys = [...IDENTITY8, 'token', 'generation'];
function operation(v, kind, view) {
	header(v, kind);
	identity8(v);
	decimal8(v.token, 1n);
	decimal8(v.generation, 1n);
	bind8(v, view, operationKeys);
}
export function validatePatch8(v, view, open, ordinal) {
	validateView8(view, open);
	shape8(v, [
		'kind',
		'protocol',
		'id',
		...operationKeys,
		'ordinal',
		'patch',
		'rgba',
	]);
	operation(v, 'patch', view);
	rect8(v.patch, geometry8(view.visible, view.level, open.canvas).size);
	if (
		v.id !== view.id ||
		v.ordinal !== ordinal ||
		!equal8(v.patch, patchRect8(view, open.canvas, ordinal))
	)
		fail8('ConflictStaleBase', 'Protocol8 exact row-major patch');
	const p = patchRect8(view, open.canvas, ordinal);
	buffer8(v.rgba, (p[2] - p[0]) * (p[3] - p[1]) * 4);
}
export function validateAck8(v, view, open, ordinal) {
	validateView8(view, open);
	shape8(v, ['kind', 'protocol', 'id', ...operationKeys, 'ordinal', 'patch']);
	operation(v, 'ack', view);
	rect8(v.patch, geometry8(view.visible, view.level, open.canvas).size);
	if (
		v.ordinal !== ordinal ||
		!equal8(v.patch, patchRect8(view, open.canvas, ordinal))
	)
		fail8('ConflictStaleBase', 'Protocol8 copied patch ACK');
}
export function validateComplete8(v, view, open) {
	validateView8(view, open);
	shape8(v, ['kind', 'protocol', 'id', ...operationKeys, 'patchCount']);
	operation(v, 'complete', view);
	if (
		v.id !== view.id ||
		v.patchCount !==
			geometry8(view.visible, view.level, open.canvas).patches
	)
		fail8('ConflictStaleBase', 'Protocol8 COMPLETE binding');
}
export function validateCancel8(v, view, open) {
	validateView8(view, open);
	shape8(v, ['kind', 'protocol', 'id', ...operationKeys]);
	operation(v, 'cancel', view);
}
export function validateCancelled8(v, cancel, view, open) {
	validateCancel8(cancel, view, open);
	shape8(v, ['kind', 'protocol', 'id', ...operationKeys]);
	operation(v, 'cancelled', view);
	bind8(v, cancel, ['id']);
}
export function validateClose8(v, open, token) {
	validateOpen8(open);
	shape8(v, ['kind', 'protocol', 'id', ...IDENTITY8, 'token']);
	header(v, 'close');
	identity8(v);
	bind8(v, open);
	decimal8(v.token);
	if (v.token !== token) fail8('ConflictStaleBase', 'Protocol8 CLOSE token');
}
export function validateClosed8(v, close, open) {
	validateClose8(close, open, close.token);
	shape8(v, ['kind', 'protocol', 'id', ...IDENTITY8, 'token']);
	header(v, 'closed');
	identity8(v);
	bind8(v, close, [...IDENTITY8, 'token', 'id']);
	// A valid shape is not evidence tasks/factories are joined: runtime owns that.
}
