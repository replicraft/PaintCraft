// Strict failed-OPEN/operation cleanup reply, never a fabricated OPENED/CLOSED.
import {
	IDENTITY8,
	shape8,
	identity8,
	bind8,
	decimal8,
	uint8,
	fail8,
} from './layer-blend-preview-common.js';
import { typedRustError8 } from './layer-blend-preview-wire.js';
export function binding8(open, view) {
	return Object.fromEntries([
		...IDENTITY8.map((k) => [k, open[k]]),
		['token', view?.token ?? '0'],
		['generation', view?.generation ?? '0'],
	]);
}
export function sourceIdentity8(open) {
	return Object.fromEntries(IDENTITY8.map((k) => [k, open[k]]));
}
export function validateFailure8(value, request, open, view) {
	shape8(value, [
		'kind',
		'protocol',
		'id',
		'phase',
		'closed',
		'detail',
		'binding',
	]);
	if (
		value.kind !== 'error' ||
		value.protocol !== 8 ||
		value.id !== request.id ||
		value.closed !== true ||
		!['open', 'view', 'ack', 'cancel', 'close'].includes(value.phase) ||
		value.phase !== request.kind
	)
		fail8('CodecFailed', 'Source8 joined failure correlation');
	uint8(value.id, 0xffffffff, 1);
	shape8(value.detail, ['code', 'message', 'operationIndex']);
	if (!typedRustError8(JSON.stringify(value.detail)))
		fail8('CodecFailed', 'Source8 typed failure detail');
	shape8(value.binding, [...IDENTITY8, 'token', 'generation']);
	identity8(value.binding);
	decimal8(value.binding.token);
	decimal8(value.binding.generation);
	bind8(value.binding, binding8(open, view), [
		...IDENTITY8,
		'token',
		'generation',
	]);
	// closed=true is emitted only by the trusted runtime after genuine consuming
	// close6 (or typed constructor RAII/no-instance proof), never a live-count guess.
}
