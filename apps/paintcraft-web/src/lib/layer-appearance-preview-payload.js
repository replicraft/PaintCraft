// Source7 transport metadata/visual intent only. Rust owns source/math/admission.
import {
	geometry7,
	sha7,
	shape7,
	validateBegin7,
} from './filter-preview-appearance-protocol.js';
import { validateCopySpec } from './filter-preview-copy.js';
import { fail, OUTPUT_PIXELS_MAX } from './filter-preview-protocol.js';

const rustCodes = new Set([
	'input.schema',
	'policy.unknown',
	'reference.unknown_alias',
	'reference.unknown_id',
	'target.invalid',
	'capability.unsupported',
	'conflict.stale_base',
	'resource.limit',
	'operation.cancelled',
	'codec.failed',
	'storage.failed',
	'invariant.violation',
]);
export function json7(raw, max) {
	if (
		typeof raw !== 'string' ||
		raw.length > max ||
		new TextEncoder().encode(raw).length > max
	)
		throw fail('CodecFailed', 'Source7 Rust JSON extent');
	return JSON.parse(raw);
}
export function error7(error, completed = false) {
	let rust = null;
	if (typeof error === 'string' && error.length <= 4096) {
		try {
			const value = JSON.parse(error);
			if (
				rustCodes.has(value?.code) &&
				typeof value.message === 'string' &&
				value.message.length <= 4096
			)
				rust = value;
		} catch {
			/* Unknown/trap strings do not attest Rust completion. */
		}
	}
	return {
		completed: completed && rust !== null,
		code:
			rust?.code ??
			(typeof error?.code === 'string'
				? error.code.slice(0, 64)
				: 'CodecFailed'),
		message:
			rust?.message ??
			(typeof error?.message === 'string'
				? error.message.slice(0, 4096)
				: 'Source7 Worker failed'),
	};
}
export function generation7(value) {
	if (
		typeof value !== 'string' ||
		!/^(0|[1-9]\d{0,19})$/.test(value) ||
		BigInt(value) > 0xffffffffffffffffn
	)
		throw fail('InputSchema', 'Source7 cancel generation');
	return BigInt(value);
}
export function sourceBuffer7(value, maximum) {
	if (
		!(value instanceof ArrayBuffer) ||
		value.byteLength < 1 ||
		value.byteLength > maximum ||
		value.resizable === true
	)
		throw fail(
			'ResourceLimit',
			'Source7 full fixed source ArrayBuffer required',
		);
	return value;
}
export function visualSpec7(output, request, plan) {
	validateBegin7(request, plan);
	const encoded = validateCopySpec(output, request, plan);
	if (
		!['print', 'clipboard'].includes(output.purpose) ||
		output.frontend !== 'studio'
	)
		throw fail(
			'CapabilityUnsupported',
			'Source7 foreign downloads remain closed',
		);
	// Selection indices name source pixels. Do not let display clipping shift them.
	const [x, y, w, h] = request.view.region;
	if (x < 0 || y < 0 || x + w > plan.size[0] || y + h > plan.size[1])
		throw fail(
			'InputSchema',
			'Source7 visual copy must be inside its source',
		);
	const actual = geometry7(request.view, plan.size);
	if (actual.width * actual.height > OUTPUT_PIXELS_MAX)
		throw fail('ResourceLimit', 'Source7 visual output extent');
	return encoded;
}
export function message7(value, fields, lastId) {
	shape7(value, ['kind', 'id', ...fields]);
	if (
		!Number.isInteger(value.id) ||
		value.id < 1 ||
		value.id > 0xffffffff ||
		value.id <= lastId
	)
		throw fail('InputSchema', 'Source7 monotonic message id');
}
export function selection7(value, expected, digest) {
	sha7(digest);
	if (
		!(value instanceof ArrayBuffer) ||
		value.resizable === true ||
		value.byteLength !== expected
	)
		throw fail('ConflictStaleBase', 'Source7 selection buffer');
	return value;
}
export function pngExtent7(bytes, binding) {
	const signature = [137, 80, 78, 71, 13, 10, 26, 10];
	if (
		!(bytes instanceof Uint8Array) ||
		bytes.byteLength < 33 ||
		signature.some((b, i) => bytes[i] !== b)
	)
		throw fail('CodecFailed', 'Source7 visual PNG signature');
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	if (
		view.getUint32(8) !== 13 ||
		String.fromCharCode(...bytes.subarray(12, 16)) !== 'IHDR' ||
		view.getUint32(16) !== binding.width ||
		view.getUint32(20) !== binding.height ||
		bytes[24] !== 8 ||
		bytes[25] !== 6
	)
		throw fail('CodecFailed', 'Source7 visual PNG geometry');
}
