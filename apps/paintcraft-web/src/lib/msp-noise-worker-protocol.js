import {
	freezeNoiseContexts,
	freezeNoiseWorkload,
	verifyNoiseWorkload,
} from './source-noise-export-context.js';
// Version-aware scoped Noise protocol8; oldMSP4..7 and DDS8 stay independent.
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
export const MSP_SOURCE_MAX = 32 * 1024 * 1024 + 128 * 1024;
export const MSP_ENCODED_MAX = 1024 * 1024;
export const MSP_PREVIEW_MAX = 192 * 192 * 4;
export const MSP_PROTOCOL_VERSION = 8;
export const MSP_REQUEST_HEADER = 704;
export const MSP_RESPONSE_HEADER = 864;
export const MSP_OUTPUT_MAX =
	MSP_RESPONSE_HEADER + MSP_ENCODED_MAX + MSP_PREVIEW_MAX + 4096;
export const MSP_RESERVATION = 512 * 1024 * 1024;

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
		throw refusal('CodecFailed', 'Invalid browser MSP UTF-8');
	}
}
const initialPlans = new WeakSet();
// New version-aware authority; never wrap the legacy version2-only parser.
export function freezeInitialMspPlan(plan) {
	if (plan && initialPlans.has(plan)) return plan;
	if (
		!plan ||
		plan.protocolVersion !== 8 ||
		![1, 2].includes(plan.version) ||
		!Number.isInteger(plan.width) ||
		!Number.isInteger(plan.height) ||
		plan.width < 1 ||
		plan.height < 1 ||
		plan.width > 65535 ||
		plan.height > 65535 ||
		plan.width * plan.height > 4194304 ||
		!Number.isInteger(plan.sourceFlags) ||
		plan.sourceFlags < 0 ||
		plan.sourceFlags > 15 ||
		![0, 1].includes(plan.bakeIntent) ||
		!(plan.filterDigest instanceof Uint8Array) ||
		plan.filterDigest.length !== 32
	)
		throw refusal('InputSchema', 'Invalid browser MSP8 profile');
	const frozen = Object.freeze({
		protocolVersion: 8,
		width: plan.width,
		height: plan.height,
		version: plan.version,
		sourceFlags: plan.sourceFlags,
		bakeIntent: plan.bakeIntent,
		filterDigest: Object.freeze([...plan.filterDigest]),
		metadataContext: freezeMetadataContext(plan.metadataContext, 2),
		maskContext: freezeMaskContext(plan.maskContext, plan.sourceFlags),
		noiseWorkload: freezeNoiseWorkload(plan.noiseWorkload),
	});
	initialPlans.add(frozen);
	return frozen;
}
export function freezeMspPlan(plan) {
	if (plan && frozenPlans.has(plan)) return plan;
	const authority = freezeInitialMspPlan(plan);
	const contexts = freezeNoiseContexts(
		plan.noiseRegionContext,
		plan.noiseProgramContext,
		new Uint8Array(authority.noiseWorkload),
		160 * 1024 * 1024,
	);
	const frozen = Object.freeze({
		...authority,
		noiseRegionContext: contexts.region,
		noiseProgramContext: contexts.program,
		noiseWorkload: contexts.workload,
	});
	frozenPlans.add(frozen);
	return frozen;
}
export async function captureMsp(
	source,
	owner,
	plan,
	nonce,
	crypto = globalThis.crypto,
) {
	if (
		!(source instanceof Uint8Array) ||
		!source.length ||
		source.length > MSP_SOURCE_MAX
	)
		throw refusal('ResourceLimit', 'Browser MSP snapshot allowance');
	plan = freezeMspPlan(plan);
	const frozen = source.slice();
	await verifyNoiseWorkload(new Uint8Array(plan.noiseWorkload), crypto);
	if (
		new DataView(
			new Uint8Array(plan.noiseRegionContext).buffer,
		).getBigUint64(72) !== BigInt(frozen.length)
	)
		throw refusal('InputSchema', 'MSP8 actual source length');
	const header = new Uint8Array(MSP_REQUEST_HEADER);
	header.set(encoder.encode('PCMSPQ08'));
	const view = new DataView(header.buffer);
	view.setUint16(8, MSP_PROTOCOL_VERSION);
	view.setUint32(12, frozen.length);
	view.setUint32(16, plan.width);
	view.setUint32(20, plan.height);
	header.set(ownerBytes(owner), 24);
	header.set(nonceBytes(nonce), 65);
	header.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', frozen)),
		81,
	);
	header[113] = plan.version;
	header[121] = plan.sourceFlags;
	header[122] = plan.bakeIntent;
	header.set(plan.filterDigest, 128);
	header.set(plan.metadataContext, 160);
	header.set(plan.maskContext, 224);
	header.set(plan.noiseRegionContext, 256);
	header.set(plan.noiseProgramContext, 384);
	header.set(plan.noiseWorkload, 640);
	const captured = { source: frozen, header, plan };
	proofs.set(captured, {});
	return captured;
}
export function encodeMspRequest(captured) {
	const bytes = new Uint8Array(MSP_REQUEST_HEADER + captured.source.length);
	bytes.set(captured.header);
	bytes.set(captured.source, MSP_REQUEST_HEADER);
	return bytes;
}
export function isCompletedMspRefusal(error, captured) {
	return (
		captured &&
		error !== null &&
		(typeof error === 'object' || typeof error === 'function') &&
		proofs.has(captured) &&
		refusals.get(error) === proofs.get(captured)
	);
}
export async function decodeMspResponse(
	bytes,
	captured,
	current,
	version,
	awaitBounded = (p) => p,
) {
	if (!matchesOwner(current, captured.header.subarray(24, 65)))
		throw refusal('ConflictStaleBase', 'Browser MSP owner changed');
	require(bytes instanceof Uint8Array &&
		bytes.length >= MSP_RESPONSE_HEADER &&
		bytes.length <= MSP_OUTPUT_MAX, 'Browser MSP response allowance');
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
	require(equal(bytes.subarray(0, 8), encoder.encode('PCMSPR08')) &&
		view.getUint16(8) === MSP_PROTOCOL_VERSION &&
		status <= 5 &&
		!bytes[11] &&
		bytes
			.subarray(267, 288)
			.every((b) => b === 0), 'Browser MSP response framing');
	require(width === captured.plan.width &&
		height === captured.plan.height &&
		view.getUint32(20) === captured.source.length &&
		equal(bytes.subarray(32, 73), captured.header.subarray(24, 65)) &&
		equal(bytes.subarray(73, 89), captured.header.subarray(65, 81)) &&
		equal(bytes.subarray(89, 121), captured.header.subarray(81, 113)) &&
		bytes[121] === captured.plan.version &&
		bytes.subarray(122, 129).every((b) => b === 0) &&
		bytes.subarray(194, 198).every((b) => b === 0) &&
		bytes[265] === captured.plan.sourceFlags &&
		bytes[266] === captured.plan.bakeIntent &&
		equal(bytes.subarray(288, 320), captured.header.subarray(128, 160)) &&
		equal(bytes.subarray(320, 384), captured.header.subarray(160, 224)) &&
		equal(bytes.subarray(384, 416), captured.header.subarray(224, 256)) &&
		equal(bytes.subarray(416, 544), captured.header.subarray(256, 384)) &&
		equal(bytes.subarray(544, 800), captured.header.subarray(384, 640)) &&
		equal(bytes.subarray(800, 864), captured.header.subarray(640, 704)) &&
		versionLength > 0 &&
		versionLength <= 64 &&
		utf8(bytes.subarray(199, 199 + versionLength)) === version &&
		bytes.subarray(199 + versionLength, 263).every((b) => b === 0) &&
		encoded <= MSP_ENCODED_MAX &&
		preview <= MSP_PREVIEW_MAX &&
		detail <= 4096 &&
		MSP_RESPONSE_HEADER + encoded + preview + detail ===
			bytes.length, 'Browser MSP response binding/length/version');
	const encodedBytes = bytes.subarray(
		MSP_RESPONSE_HEADER,
		MSP_RESPONSE_HEADER + encoded,
	);
	const previewBytes = bytes.subarray(
		MSP_RESPONSE_HEADER + encoded,
		MSP_RESPONSE_HEADER + encoded + preview,
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
		), 'Browser MSP result digest');
	if (status) {
		require(!encoded &&
			!preview &&
			!pw &&
			!ph &&
			!mask &&
			!count, 'Browser MSP refusal carries output');
		const error = refusal(
			[
				'',
				'CapabilityUnsupported',
				'CodecFailed',
				'ResourceLimit',
				'InputSchema',
				'OperationCancelled',
			][status],
			utf8(bytes.subarray(MSP_RESPONSE_HEADER)),
		);
		refusals.set(error, proofs.get(captured));
		throw error;
	}
	const side = Math.max(width, height),
		scale = Math.min(side, 192);
	const expectedW = Math.max(1, Math.floor((width * scale) / side)),
		expectedH = Math.max(1, Math.floor((height * scale) / side));

	require(encoded > 0 &&
		!detail &&
		pw === expectedW &&
		ph === expectedH &&
		mask <= 15 &&
		(mask & 3) === (captured.plan.sourceFlags & 3) &&
		Boolean(mask & 8) === Boolean(captured.plan.sourceFlags & 4) &&
		(!(captured.plan.sourceFlags & 4) || captured.plan.bakeIntent === 1) &&
		Boolean(mask & 4) ===
			metadataOmitted(
				captured.plan.metadataContext,
			), 'Browser MSP8 result profile');
	return Object.freeze({
		encoded: encodedBytes,
		preview: previewBytes,
		width,
		height,
		previewWidth: pw,
		previewHeight: ph,
		lossMask: mask,
	});
}
