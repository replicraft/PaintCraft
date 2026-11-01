// Explicit flat-output1. No new Worker/pool; caller supplies existing slot transport.

import { validateResult6 } from './filter-preview-noise-protocol.js';
import { encodeProgram } from './filter-preview-program-protocol.js';
import {
	canonical,
	fail,
	sha,
	validateRequest,
	validateResult,
} from './filter-preview-protocol.js';
export const COPY_LIMITS = Object.freeze({
	version: 1,
	outputBytes: 16 << 20,
	scratchBytes: 32 << 20,
	selectionBytes: 4194304,
	workPerPixel: 64,
	selectionWorkPerByte: 2,
	maxSide: 8192,
});
function shape(value, keys) {
	if (
		!value ||
		typeof value !== 'object' ||
		Array.isArray(value) ||
		Object.getPrototypeOf(value) !== Object.prototype ||
		Object.keys(value).length !== keys.length ||
		keys.some((k) => !Object.hasOwn(value, k))
	)
		throw fail('InputSchema', 'Flat output fields');
}
export function validateCopySpec(s, request, plan) {
	shape(s, [
		'version',
		'purpose',
		'format',
		'quality',
		'matte',
		'frontend',
		'omitMetadata',
		'bakeFilters',
		'maskLength',
		'maskSha256',
	]);
	if (s.version !== 1 || !['png', 'jpeg', 'bmp'].includes(s.format))
		throw fail(
			'CapabilityUnsupported',
			'Unpriced flat output codec/version',
		);
	if (
		!['download', 'print', 'clipboard'].includes(s.purpose) ||
		!['classic-v7', 'classic-v10', 'classic-v11', 'studio'].includes(
			s.frontend,
		) ||
		!Number.isInteger(s.quality) ||
		s.quality < 1 ||
		s.quality > 100 ||
		!Array.isArray(s.matte) ||
		s.matte.length !== 3 ||
		s.matte.some((v) => !Number.isInteger(v) || v < 0 || v > 255) ||
		typeof s.omitMetadata !== 'boolean' ||
		typeof s.bakeFilters !== 'boolean' ||
		!Number.isSafeInteger(s.maskLength) ||
		s.maskLength < 0 ||
		s.maskLength > COPY_LIMITS.selectionBytes ||
		typeof s.maskSha256 !== 'string' ||
		!/^[a-f0-9]{64}$/.test(s.maskSha256)
	)
		throw fail('InputSchema', 'Flat output options');
	const [x, y, w, h] = request.view.region;
	if (
		request.operation !== null ||
		request.target !== null ||
		request.render !== true ||
		request.view.level !== 0 ||
		w > COPY_LIMITS.maxSide ||
		h > COPY_LIMITS.maxSide
	)
		throw fail(
			'CapabilityUnsupported',
			'Flat output requires bounded level0 BASE',
		);
	if (
		s.purpose !== 'clipboard' &&
		(x !== 0 ||
			y !== 0 ||
			w !== plan.size[0] ||
			h !== plan.size[1] ||
			s.maskLength !== 0)
	)
		throw fail('InputSchema', 'Flat output whole canvas');
	if (s.maskLength !== 0 && s.maskLength !== w * h)
		throw fail('InputSchema', 'Flat output mask geometry');
	if (
		(s.purpose !== 'download' &&
			(s.format !== 'png' ||
				s.quality !== 90 ||
				s.matte.some((x) => x !== 255) ||
				s.omitMetadata ||
				s.bakeFilters)) ||
		(s.format !== 'jpeg' && s.matte.some((x) => x !== 255))
	)
		throw fail('InputSchema', 'Flat output policies');
	return encodeProgram(s, 4096);
}
export function validateCopyLimits(limits) {
	shape(limits, Object.keys(COPY_LIMITS));
	for (const key of Object.keys(COPY_LIMITS))
		if (limits[key] !== COPY_LIMITS[key])
			throw fail('CapabilityUnsupported', 'Flat output artifact limits');
}
export function validateCopyBinding(binding, request, plan) {
	// A separate encoded envelope has no RGBA transfer. Reuse the complete
	// Source6 metadata validator with its non-pixel metadata projection only.
	validateResult6(
		{ ...binding, kind: 'prepared', id: binding.id ?? 1 },
		request,
		plan,
		validateResult,
	);
}
export async function validateEncodedCopy(
	result,
	request,
	plan,
	spec,
	outputSha256,
) {
	shape(result, [
		'kind',
		'id',
		'outputVersion',
		'outputSha256',
		'bytesSha256',
		'mime',
		'binding',
		'bytes',
	]);
	if (
		result.kind !== 'encoded-copy' ||
		result.outputVersion !== 1 ||
		result.outputSha256 !== outputSha256 ||
		result.mime !==
			{ png: 'image/png', jpeg: 'image/jpeg', bmp: 'image/bmp' }[
				spec.format
			] ||
		!(result.bytes instanceof Uint8Array) ||
		!result.bytes.length ||
		result.bytes.length > COPY_LIMITS.outputBytes - spec.maskLength
	)
		throw fail('CodecFailed', 'Flat output result');
	validateCopyBinding(result.binding, request, plan);
	if ((await sha(result.bytes)) !== result.bytesSha256)
		throw fail('CodecFailed', 'Flat output byte digest');
}
export function makeEncodedCopy({
	plan,
	send,
	start,
	finish,
	hold,
	markUpdate,
	checkCurrent,
}) {
	return async (options, input, selection = new Uint8Array()) => {
		validateRequest(options, plan);
		if (plan.protocol !== 6 || !(selection instanceof Uint8Array))
			throw fail('CapabilityUnsupported', 'Flat output requires Source6');
		const spec = JSON.parse(validateCopySpec(input, options, plan));
		if (
			selection.length !== spec.maskLength ||
			(await sha(selection)) !== spec.maskSha256
		)
			throw fail('ConflictStaleBase', 'Flat output selection changed');
		const request = start(options);
		let success = false;
		try {
			const outputSha256 = await sha(
				new TextEncoder().encode(canonical(spec)),
			);
			checkCurrent(request);
			const admitted = await send('copy-prepare', {
				request,
				output: spec,
			});
			shape(admitted, [
				'kind',
				'id',
				'outputVersion',
				'outputSha256',
				'binding',
				'limits',
			]);
			if (
				admitted.kind !== 'copy-admitted' ||
				admitted.outputVersion !== 1 ||
				admitted.outputSha256 !== outputSha256
			)
				throw fail('CodecFailed', 'Flat output admission');
			validateCopyLimits(admitted.limits);
			validateCopyBinding(
				admitted.binding,
				{
					...request,
					operationSha256: await sha(
						new TextEncoder().encode('null'),
					),
				},
				plan,
			);
			checkCurrent(request);
			const mask = selection.slice();
			const pending = send(
				'copy-input',
				{ outputSha256, mask: mask.buffer },
				[mask.buffer],
			);
			markUpdate();
			const result = await pending;
			await validateEncodedCopy(
				result,
				{
					...request,
					operationSha256: await sha(
						new TextEncoder().encode('null'),
					),
				},
				plan,
				spec,
				outputSha256,
			);
			// A cancel/close may have arrived while byteSHA was pending.
			checkCurrent(request);
			hold();
			let released = false;
			result.release = () => {
				if (!released) {
					released = true;
					finish();
				}
			};
			success = true;
			return result;
		} finally {
			if (!success) finish();
		}
	};
}

// The barrier is installed synchronously, including preflight/selection SHA.
// Close cancels first, then joins this whole operation before sending close or
// releasing the admitted slot. Rejection is consumed only by the join barrier;
// the public operation still rejects to its actual caller.
export function joinableCopy(run) {
	let pending = null;
	return {
		run(...args) {
			if (pending)
				return Promise.reject(
					fail('ResourceLimit', 'Flat output finalization pending'),
				);
			const result = Promise.resolve().then(() => run(...args));
			const joined = result.then(
				() => {},
				() => {},
			);
			pending = joined;
			joined.then(() => {
				if (pending === joined) pending = null;
			});
			return result;
		},
		join() {
			return pending ?? Promise.resolve();
		},
	};
}
