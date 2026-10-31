import {
	freezeProgramSpatialContext,
	freezeProgramContext,
} from './source-program-worker-context.js';
// Browser-only generic prepared protocol7. Native/JXL decoder2 stays independent.
import {
	equal,
	matchesOwner,
	nonceBytes,
	ownerBytes,
	refusal,
} from './codec-worker-protocol.js';
import {
	freezeMetadataContext,
	metadataOmitted,
} from './source-metadata-worker-context.js';
export const DDS_SOURCE_MAX = 32 * 1024 * 1024;
export const DDS_ENCODED_MAX = 24 * 1024 * 1024 + 4096;
export const DDS_PREVIEW_MAX = 192 * 192 * 4;
export const DDS_PROTOCOL_VERSION = 7;
export const DDS_REQUEST_HEADER = 448;
export const DDS_RESPONSE_HEADER = 608;
export const DDS_OUTPUT_MAX =
	DDS_RESPONSE_HEADER + DDS_ENCODED_MAX + DDS_PREVIEW_MAX + 4096;
export const DDS_RESERVATION = 640 * 1024 * 1024;

import { freezeMaskContext } from './source-mask-worker-context.js';

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const frozenPlans = new WeakSet();
const proofs = new WeakMap();
const refusals = new WeakMap();
function require(value, message) {
	if (!value) throw refusal('CodecFailed', message);
}
function utf8(bytes) {
	try {
		return decoder.decode(bytes);
	} catch {
		throw refusal('CodecFailed', 'Invalid browser DDS UTF-8');
	}
}
export function freezeDdsPlan(plan) {
	if (plan && frozenPlans.has(plan)) return plan;
	if (!plan) throw refusal('InputSchema', 'Invalid browser DDS profile');
	const {
		width,
		height,
		sourceFlags,
		settings,
		metadataContext,
		maskContext,
		filterDigest,
		bakeIntent,
		protocolVersion,
		spatialContext,
		programContext,
	} = plan;
	if (
		protocolVersion !== 7 ||
		!Number.isInteger(width) ||
		!Number.isInteger(height) ||
		width < 1 ||
		height < 1 ||
		width > 65536 ||
		height > 65536 ||
		width * height > 4194304 ||
		!Number.isInteger(sourceFlags) ||
		sourceFlags < 0 ||
		sourceFlags > 15 ||
		!Number.isInteger(bakeIntent) ||
		bakeIntent < 0 ||
		bakeIntent > 1 ||
		!(filterDigest instanceof Uint8Array) ||
		filterDigest.length !== 32 ||
		!(settings instanceof Uint8Array) ||
		settings.length !== 8
	)
		throw refusal('InputSchema', 'Invalid browser DDS profile');
	const s = settings.slice();
	const levels = 32 - Math.clz32(Math.max(width, height));
	if (
		s[0] > 3 ||
		s[1] > 1 ||
		s[2] !== (s[0] === 1 ? 1 : 0) ||
		s[3] < 1 ||
		s[3] > levels ||
		s[4] > 1 ||
		s[5] > 1 ||
		s[6] ||
		s[7] ||
		(s[3] === 1 && s[4]) ||
		(s[0] === 0 && s[5])
	)
		throw refusal('InputSchema', 'Noncanonical browser DDS settings');
	const metadata = freezeMetadataContext(metadataContext, 1);
	const spatial = freezeProgramSpatialContext(
		spatialContext,
		192 * 1024 * 1024,
	);
	const programs = freezeProgramContext(
		programContext,
		spatial,
		192 * 1024 * 1024,
	);
	const frozen = Object.freeze({
		width,
		height,
		sourceFlags,
		metadataContext: metadata,
		maskContext: freezeMaskContext(maskContext, sourceFlags),
		filterDigest: Object.freeze(
			Array.from({ length: 32 }, (_, index) => filterDigest[index]),
		),
		bakeIntent,
		protocolVersion: 7,
		spatialContext: spatial,
		programContext: programs,
		settings: Object.freeze([...s]),
	});
	frozenPlans.add(frozen);
	return frozen;
}
export async function captureDds(
	source,
	owner,
	plan,
	nonce,
	crypto = globalThis.crypto,
) {
	if (
		!(source instanceof Uint8Array) ||
		!source.length ||
		source.length > DDS_SOURCE_MAX
	)
		throw refusal('ResourceLimit', 'Browser DDS snapshot allowance');
	plan = freezeDdsPlan(plan);
	const frozen = source.slice();
	const header = new Uint8Array(DDS_REQUEST_HEADER);
	header.set(encoder.encode('PCDDSQ07'));
	const view = new DataView(header.buffer);
	view.setUint16(8, DDS_PROTOCOL_VERSION);
	view.setUint32(12, frozen.length);
	view.setUint32(16, plan.width);
	view.setUint32(20, plan.height);
	header.set(ownerBytes(owner), 24);
	header.set(nonceBytes(nonce), 65);
	header.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', frozen)),
		81,
	);
	header.set(plan.settings, 113);
	header[121] = plan.sourceFlags;
	header[122] = plan.bakeIntent;
	header.set(plan.filterDigest, 128);
	header.set(plan.metadataContext, 160);
	header.set(plan.maskContext, 224);
	header.set(plan.spatialContext, 256);
	header.set(plan.programContext, 320);
	const captured = { source: frozen, header, plan };
	proofs.set(captured, {});
	return captured;
}
export function encodeDdsRequest(captured) {
	const bytes = new Uint8Array(DDS_REQUEST_HEADER + captured.source.length);
	bytes.set(captured.header);
	bytes.set(captured.source, DDS_REQUEST_HEADER);
	return bytes;
}
export function isCompletedDdsRefusal(error, captured) {
	return (
		captured &&
		error !== null &&
		(typeof error === 'object' || typeof error === 'function') &&
		proofs.has(captured) &&
		refusals.get(error) === proofs.get(captured)
	);
}
export async function decodeDdsResponse(
	bytes,
	captured,
	current,
	version,
	awaitBounded = (p) => p,
) {
	if (!matchesOwner(current, captured.header.subarray(24, 65)))
		throw refusal('ConflictStaleBase', 'Browser DDS owner changed');
	require(bytes instanceof Uint8Array &&
		bytes.length >= DDS_RESPONSE_HEADER &&
		bytes.length <= DDS_OUTPUT_MAX, 'Browser DDS response allowance');
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const status = bytes[10],
		encoded = view.getUint32(24),
		pw = view.getUint16(28),
		ph = view.getUint16(30);
	const width = view.getUint32(12),
		height = view.getUint32(16),
		preview = pw * ph * 4;
	const mask = bytes[193],
		count = view.getUint32(194),
		versionLength = bytes[198],
		detail = view.getUint16(263);
	require(equal(bytes.subarray(0, 8), encoder.encode('PCDDSR07')) &&
		view.getUint16(8) === DDS_PROTOCOL_VERSION &&
		status <= 5 &&
		!bytes[11] &&
		bytes
			.subarray(267, 288)
			.every((b) => b === 0), 'Browser DDS response framing');
	require(width === captured.plan.width &&
		height === captured.plan.height &&
		view.getUint32(20) === captured.source.length &&
		equal(bytes.subarray(32, 73), captured.header.subarray(24, 65)) &&
		equal(bytes.subarray(73, 89), captured.header.subarray(65, 81)) &&
		equal(bytes.subarray(89, 121), captured.header.subarray(81, 113)) &&
		equal(bytes.subarray(121, 129), captured.header.subarray(113, 121)) &&
		bytes[265] === captured.plan.sourceFlags &&
		bytes[266] === captured.plan.bakeIntent &&
		equal(bytes.subarray(288, 320), captured.header.subarray(128, 160)) &&
		equal(bytes.subarray(320, 384), captured.header.subarray(160, 224)) &&
		equal(bytes.subarray(384, 416), captured.header.subarray(224, 256)) &&
		equal(bytes.subarray(416, 480), captured.header.subarray(256, 320)) &&
		equal(bytes.subarray(480, 608), captured.header.subarray(320, 448)) &&
		versionLength > 0 &&
		versionLength <= 64 &&
		utf8(bytes.subarray(199, 199 + versionLength)) === version &&
		bytes.subarray(199 + versionLength, 263).every((b) => b === 0) &&
		encoded <= DDS_ENCODED_MAX &&
		preview <= DDS_PREVIEW_MAX &&
		detail <= 4096 &&
		DDS_RESPONSE_HEADER + encoded + preview + detail ===
			bytes.length, 'Browser DDS response binding/length/version');
	const encodedBytes = bytes.subarray(
		DDS_RESPONSE_HEADER,
		DDS_RESPONSE_HEADER + encoded,
	);
	const previewBytes = bytes.subarray(
		DDS_RESPONSE_HEADER + encoded,
		DDS_RESPONSE_HEADER + encoded + preview,
	);
	const encodedDigest = new Uint8Array(
		await awaitBounded(crypto.subtle.digest('SHA-256', encodedBytes)),
	);
	const previewDigest = new Uint8Array(
		await awaitBounded(crypto.subtle.digest('SHA-256', previewBytes)),
	);
	require(equal(encodedDigest, bytes.subarray(129, 161)) &&
		equal(
			previewDigest,
			bytes.subarray(161, 193),
		), 'Browser DDS result digest');
	if (status) {
		require(!encoded &&
			!preview &&
			!pw &&
			!ph &&
			!mask &&
			!count, 'Browser DDS refusal carries output');
		const error = refusal(
			[
				'',
				'CapabilityUnsupported',
				'CodecFailed',
				'ResourceLimit',
				'InputSchema',
				'OperationCancelled',
			][status],
			utf8(bytes.subarray(DDS_RESPONSE_HEADER)),
		);
		refusals.set(error, proofs.get(captured));
		throw error;
	}
	const side = Math.max(width, height),
		scale = Math.min(side, 192);
	const expectedW = Math.max(1, Math.floor((width * scale) / side)),
		expectedH = Math.max(1, Math.floor((height * scale) / side));
	const s = captured.plan.settings;
	require(encoded > 0 &&
		!detail &&
		pw === expectedW &&
		ph === expectedH &&
		mask <= 127 &&
		Boolean(mask & 32) === metadataOmitted(captured.plan.metadataContext) &&
		(mask & 3) === (captured.plan.sourceFlags & 3) &&
		Boolean(mask & 64) === Boolean(captured.plan.sourceFlags & 4) &&
		(!(captured.plan.sourceFlags & 4) || captured.plan.bakeIntent === 1) &&
		Boolean(mask & 4) === (s[0] !== 0) &&
		(!(mask & 8) || (s[4] === 1 && s[3] > 1)) &&
		count <= width * height * 3 &&
		Boolean(mask & 16) === count > 0 &&
		(s[1] !== 0 || !count), 'Browser DDS result profile');
	return Object.freeze({
		encoded: encodedBytes,
		preview: previewBytes,
		width,
		height,
		previewWidth: pw,
		previewHeight: ph,
		lossMask: mask,
		quantizedRgbSamples: count,
	});
}
