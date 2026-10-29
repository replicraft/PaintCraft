// Separate whole-source TIFF operations. No decoder2 fields carry a page index.
import {
	equal,
	matchesOwner,
	nonceBytes,
	ownerBytes,
	refusal,
} from './codec-worker-protocol.js';
export const TIFF_PROTOCOL_VERSION = 2;
export const TIFF_REQUEST_HEADER = 176;
export const TIFF_RESPONSE_HEADER = 200;
export const TIFF_SOURCE_MAX = 64 * 1024 * 1024;
export const TIFF_PIXELS_MAX = 4_194_304;
export const TIFF_SIDE_MAX = 1_048_576;
export const TIFF_PAGES_MAX = 1024;
export const TIFF_ROW_BYTES = 24;
export const TIFF_REPORT_BYTES = 256;
export const TIFF_OUTPUT_MAX =
	TIFF_RESPONSE_HEADER + 64 + TIFF_REPORT_BYTES + TIFF_PIXELS_MAX * 4;
export const TIFF_RESERVATION =
	256 * 1024 * 1024 + TIFF_SOURCE_MAX * 4 + TIFF_OUTPUT_MAX * 3 + 1024 * 1024;
const text = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const proofs = new WeakMap();
const completed = new WeakMap();
const zero = (bytes) => bytes.every((value) => value === 0);
function require(value, message) {
	if (!value) throw refusal('CodecFailed', message);
}
export function isCompletedTiffRefusal(error, captured) {
	return (
		error !== null &&
		typeof error === 'object' &&
		proofs.get(captured) !== undefined &&
		completed.get(error) === proofs.get(captured)
	);
}
function hex(bytes) {
	return Array.from(bytes, (value) =>
		value.toString(16).padStart(2, '0'),
	).join('');
}
function digestBytes(value) {
	if (typeof value !== 'string' || !/^[a-f0-9]{64}$/.test(value))
		throw refusal('InputSchema', 'Invalid TIFF inventory digest');
	return Uint8Array.from(value.match(/../g), (pair) =>
		Number.parseInt(pair, 16),
	);
}
export function freezeTiffPlan(plan) {
	if (plan === null || plan === undefined) return null;
	const {
		index,
		pageCount,
		width,
		height,
		ifdOffset,
		orientation,
		sourceSha256,
	} = plan;
	if (
		!Number.isInteger(index) ||
		!Number.isInteger(pageCount) ||
		pageCount < 1 ||
		pageCount > TIFF_PAGES_MAX ||
		index < 0 ||
		index >= pageCount ||
		!Number.isInteger(orientation) ||
		orientation < 1 ||
		orientation > 8 ||
		!Number.isInteger(width) ||
		!Number.isInteger(height) ||
		width < 1 ||
		height < 1 ||
		width > TIFF_SIDE_MAX ||
		height > TIFF_SIDE_MAX ||
		width * height > TIFF_PIXELS_MAX ||
		!Number.isSafeInteger(ifdOffset) ||
		ifdOffset < 8 ||
		ifdOffset >= TIFF_SOURCE_MAX
	)
		throw refusal('InputSchema', 'Invalid explicit TIFF page selection');
	digestBytes(sourceSha256);
	return Object.freeze({
		index,
		pageCount,
		width,
		height,
		ifdOffset,
		orientation,
		sourceSha256,
	});
}
export async function captureTiff(source, owner, plan, nonce) {
	if (
		!(source instanceof Uint8Array) ||
		source.length < 1 ||
		source.length > TIFF_SOURCE_MAX
	)
		throw refusal('ResourceLimit', 'TIFF whole-source allowance');
	plan = freezeTiffPlan(plan);
	const frozen = source.slice();
	const header = new Uint8Array(TIFF_REQUEST_HEADER);
	header.set(text.encode('PCTIF002'));
	const view = new DataView(header.buffer);
	view.setUint16(8, TIFF_PROTOCOL_VERSION);
	header[10] = plan ? 2 : 1;
	view.setUint16(12, plan?.index ?? 65535);
	view.setBigUint64(16, BigInt(frozen.length));
	header.set(ownerBytes(owner), 24);
	header.set(nonceBytes(nonce ?? crypto.randomUUID()), 65);
	const sha = new Uint8Array(await crypto.subtle.digest('SHA-256', frozen));
	header.set(sha, 81);
	view.setUint32(145, TIFF_PIXELS_MAX);
	if (plan) {
		if (
			!equal(sha, digestBytes(plan.sourceSha256)) ||
			plan.ifdOffset >= source.length
		)
			throw refusal(
				'ConflictStaleBase',
				'TIFF source differs from the displayed inventory',
			);
		header.set(sha, 113);
		view.setUint16(149, plan.pageCount);
		view.setUint32(151, plan.width);
		view.setUint32(155, plan.height);
		view.setBigUint64(159, BigInt(plan.ifdOffset));
		view.setUint32(167, plan.orientation);
	}
	const captured = { source: frozen, header, plan };
	proofs.set(captured, {});
	return captured;
}
export function encodeTiffRequest(captured) {
	const out = new Uint8Array(TIFF_REQUEST_HEADER + captured.source.length);
	out.set(captured.header);
	out.set(captured.source, TIFF_REQUEST_HEADER);
	return out;
}
export async function decodeTiffResponse(
	bytes,
	captured,
	currentOwner,
	version,
	awaitBounded = (value) => value,
) {
	require(bytes instanceof Uint8Array &&
		bytes.length >= TIFF_RESPONSE_HEADER &&
		bytes.length <= TIFF_OUTPUT_MAX, 'TIFF response allowance');
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const request = new DataView(
		captured.header.buffer,
		captured.header.byteOffset,
		captured.header.byteLength,
	);
	require(equal(bytes.subarray(0, 8), text.encode('PCTIR002')) &&
		view.getUint16(8) === TIFF_PROTOCOL_VERSION &&
		bytes[10] === captured.header[10] &&
		bytes[11] <= 5 &&
		view.getUint16(12) === request.getUint16(12) &&
		zero(bytes.subarray(38, 40)) &&
		zero(bytes.subarray(193, 200)), 'TIFF response framing/selection');
	require(equal(bytes.subarray(40, 81), captured.header.subarray(24, 65)) &&
		matchesOwner(currentOwner, captured.header.subarray(24, 65)) &&
		equal(bytes.subarray(81, 97), captured.header.subarray(65, 81)) &&
		equal(
			bytes.subarray(97, 129),
			captured.header.subarray(81, 113),
		), 'TIFF response owner/nonce/source');
	const payloadLength = view.getBigUint64(24);
	const reportLength = view.getUint32(32),
		versionLength = view.getUint16(36);
	require(payloadLength <= BigInt(TIFF_PIXELS_MAX * 4) &&
		reportLength <= TIFF_REPORT_BYTES &&
		versionLength > 0 &&
		versionLength <= 64 &&
		BigInt(TIFF_RESPONSE_HEADER + versionLength + reportLength) +
			payloadLength ===
			BigInt(bytes.length), 'TIFF response extent');
	require(decoder.decode(
		bytes.subarray(
			TIFF_RESPONSE_HEADER,
			TIFF_RESPONSE_HEADER + versionLength,
		),
	) === version, 'TIFF response application version');
	const report = bytes.subarray(
		TIFF_RESPONSE_HEADER + versionLength,
		TIFF_RESPONSE_HEADER + versionLength + reportLength,
	);
	const payload = bytes.subarray(
		TIFF_RESPONSE_HEADER + versionLength + reportLength,
	);
	const reportSha = new Uint8Array(
		await awaitBounded(crypto.subtle.digest('SHA-256', report)),
	);
	const payloadSha = new Uint8Array(
		await awaitBounded(crypto.subtle.digest('SHA-256', payload)),
	);
	require(equal(reportSha, bytes.subarray(161, 193)) &&
		equal(
			payloadSha,
			bytes.subarray(129, 161),
		), 'TIFF report/payload digest');
	const pageCount = view.getUint16(14),
		width = view.getUint32(16),
		height = view.getUint32(20);
	if (bytes[11] !== 0) {
		require(pageCount === 0 &&
			width === 0 &&
			height === 0 &&
			report.length === 0 &&
			payload.length === 0, 'TIFF refusal carries pixels');
		const code = [
			'CodecFailed',
			'CodecFailed',
			'CapabilityUnsupported',
			'ResourceLimit',
			'OperationCancelled',
			'ConflictStaleBase',
		][bytes[11]];
		const error = refusal(code, 'Isolated TIFF page import refused');
		completed.set(error, proofs.get(captured));
		throw error;
	}
	if (captured.plan) {
		require(pageCount === captured.plan.pageCount &&
			width ===
				(captured.plan.orientation >= 5
					? captured.plan.height
					: captured.plan.width) &&
			height ===
				(captured.plan.orientation >= 5
					? captured.plan.width
					: captured.plan.height) &&
			report.length === TIFF_REPORT_BYTES &&
			payload.length ===
				width *
					height *
					4, 'TIFF selected result differs from displayed inventory');
	} else {
		require(width === 0 &&
			height === 0 &&
			report.length === 0 &&
			pageCount > 0 &&
			pageCount <= TIFF_PAGES_MAX &&
			payload.length ===
				pageCount * TIFF_ROW_BYTES, 'TIFF inventory framing');
		const rows = new DataView(
			payload.buffer,
			payload.byteOffset,
			payload.byteLength,
		);
		for (let index = 0; index < pageCount; index++) {
			const at = index * TIFF_ROW_BYTES,
				supported = payload[at + 2],
				rowWidth = rows.getUint32(at + 4),
				rowHeight = rows.getUint32(at + 8),
				offset = rows.getBigUint64(at + 12),
				orientation = rows.getUint32(at + 20);
			require(rows.getUint16(at) === index &&
				supported <= 1 &&
				payload[at + 3] === 0 &&
				offset >= 8n &&
				offset < BigInt(captured.source.length) &&
				(!supported ||
					(orientation >= 1 &&
						orientation <= 8 &&
						rowWidth > 0 &&
						rowHeight > 0 &&
						rowWidth <= TIFF_SIDE_MAX &&
						rowHeight <= TIFF_SIDE_MAX)), 'TIFF inventory row');
		}
	}
	return {
		pageCount,
		width,
		height,
		report,
		payload,
		sourceSha256: hex(captured.header.subarray(81, 113)),
	};
}
