// Mirrors formats::codec_worker protocol2; never publishes an editor document.
export const INPUT_BYTES_MAX = 64 * 1024 * 1024;
export const PIXELS_MAX = 4_194_304;
export const REQUEST_HEADER = 112;
export const RESPONSE_HEADER = 136;
export const OUTPUT_BYTES_MAX = PIXELS_MAX * 4 + RESPONSE_HEADER + 4096 + 64;
export const LINEAR_MEMORY_BYTES = 256 * 1024 * 1024;
const text = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
export function refusal(code, message) {
	return Object.assign(new Error(message), { code });
}
function require(condition, message) {
	if (!condition) throw refusal('CodecFailed', message);
}
function uuid(value) {
	if (
		typeof value !== 'string' ||
		!/^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(
			value,
		)
	)
		throw refusal('InputSchema', 'Invalid codec owner UUID');
	return Uint8Array.from(value.replaceAll('-', '').match(/../g), (pair) =>
		Number.parseInt(pair, 16),
	);
}
export function ownerBytes(owner) {
	if (!owner || owner.frontend !== 3)
		throw refusal('CapabilityUnsupported', 'Codec Worker is Studio-only');
	const epoch =
		typeof owner.epoch === 'bigint'
			? owner.epoch
			: Number.isSafeInteger(owner.epoch)
				? BigInt(owner.epoch)
				: -1n;
	if (epoch < 0n || epoch > 0xffffffffffffffffn)
		throw refusal('InputSchema', 'Invalid codec owner epoch');
	const bytes = new Uint8Array(41);
	bytes.set(uuid(owner.session));
	bytes.set(uuid(owner.state), 16);
	new DataView(bytes.buffer).setBigUint64(32, epoch);
	bytes[40] = owner.frontend;
	return bytes;
}
export function equal(a, b) {
	return (
		a.length === b.length && a.every((value, index) => value === b[index])
	);
}
export function matchesOwner(current, expected) {
	try {
		return equal(ownerBytes(current), expected);
	} catch {
		return false;
	}
}
export async function capture(
	source,
	owner,
	codec,
	pixels,
	crypto = globalThis.crypto,
) {
	if (
		!(source instanceof Uint8Array) ||
		!source.length ||
		source.length > INPUT_BYTES_MAX ||
		!Number.isInteger(pixels) ||
		pixels < 1 ||
		pixels > PIXELS_MAX
	)
		throw refusal('ResourceLimit', 'Codec input/pixel allowance');
	if (codec !== 1 && codec !== 2)
		throw refusal('CapabilityUnsupported', 'Unknown codec Worker profile');
	// Copy before the first await. Caller mutation never changes captured bytes.
	const frozen = source.slice();
	const header = new Uint8Array(REQUEST_HEADER);
	header.set(text.encode('PCDEC002'));
	const view = new DataView(header.buffer);
	view.setUint16(8, 2);
	header[10] = codec;
	view.setUint32(11, pixels);
	view.setBigUint64(15, BigInt(frozen.length));
	header.set(ownerBytes(owner), 23);
	header.set(crypto.getRandomValues(new Uint8Array(16)), 64);
	header.set(
		new Uint8Array(await crypto.subtle.digest('SHA-256', frozen)),
		80,
	);
	return { source: frozen, header, codec, pixels };
}
export function encodeRequest(captured) {
	const bytes = new Uint8Array(REQUEST_HEADER + captured.source.length);
	bytes.set(captured.header);
	bytes.set(captured.source, REQUEST_HEADER);
	return bytes;
}
export function decodeResponse(bytes, captured, currentOwner, version) {
	if (!matchesOwner(currentOwner, captured.header.subarray(23, 64)))
		throw refusal(
			'ConflictStaleBase',
			'Codec document/frontend owner changed',
		);
	require(bytes instanceof Uint8Array &&
		bytes.length >= RESPONSE_HEADER &&
		bytes.length <= OUTPUT_BYTES_MAX, 'Codec response size allowance');
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	require(equal(bytes.subarray(0, 8), text.encode('PCPIX002')) &&
		view.getUint16(8) === 2, 'Codec response protocol');
	const status = bytes[10];
	require(status <= 4 &&
		bytes[11] === captured.codec, 'Codec response sample model');
	if (
		!equal(bytes.subarray(47, 88), captured.header.subarray(23, 64)) ||
		!equal(bytes.subarray(88, 104), captured.header.subarray(64, 80)) ||
		!equal(bytes.subarray(104, 136), captured.header.subarray(80, 112))
	)
		throw refusal(
			'ConflictStaleBase',
			'Codec response source/job identity changed',
		);
	const width = view.getUint32(12),
		height = view.getUint32(16);
	const alpha = bytes[20],
		primaries = view.getUint16(21),
		transfer = view.getUint16(23),
		matrix = view.getUint16(25),
		range = bytes[27];
	const primary = view.getUint32(28),
		opacity = view.getUint32(32),
		detail = view.getUint16(36),
		versionBytes = bytes[38];
	// Wasm has a linear-memory bound, not the native requested-heap metric.
	require(view.getBigUint64(39) ===
		0n, 'Wasm response claims native heap measurement');
	require(alpha <= 1 &&
		range <= 1 &&
		detail <= 4096 &&
		versionBytes <= 64 &&
		RESPONSE_HEADER + versionBytes + detail + primary + opacity ===
			bytes.length, 'Codec response field lengths');
	require(decoder.decode(
		bytes.subarray(RESPONSE_HEADER, RESPONSE_HEADER + versionBytes),
	) === version, 'Codec application version mismatch');
	if (status) {
		require(width === 0 &&
			height === 0 &&
			primary === 0 &&
			opacity === 0, 'Codec failure carries pixels');
		throw Object.assign(
			refusal(
				[
					'',
					'CapabilityUnsupported',
					'CodecFailed',
					'ResourceLimit',
					'OperationCancelled',
				][status],
				decoder.decode(bytes.subarray(RESPONSE_HEADER + versionBytes)),
			),
			{ codecCompleted: true },
		);
	}
	const pixels = width * height,
		side = captured.codec === 1 ? 16384 : 65536;
	require(width > 0 &&
		height > 0 &&
		width <= side &&
		height <= side &&
		pixels <= captured.pixels &&
		pixels <= PIXELS_MAX &&
		detail === 0, 'Codec response dimensions');
	const payload = bytes.subarray(RESPONSE_HEADER + versionBytes);
	if (captured.codec === 1) {
		require(primary === pixels &&
			opacity === (alpha ? pixels : 0) &&
			range === 1 &&
			[1, 2].includes(primaries) &&
			[2, 13].includes(transfer) &&
			[0, 1, 2].includes(matrix), 'Codec native gray profile');
		return Object.freeze({
			width,
			height,
			model: 'native-gray8',
			gray: payload.subarray(0, primary),
			alpha: opacity ? payload.subarray(primary) : null,
			encoding: Object.freeze({
				primaries,
				transfer,
				matrix,
				fullRange: true,
			}),
		});
	}
	require(primary === pixels * 4 &&
		opacity === 0 &&
		alpha === 0 &&
		range === 1 &&
		primaries === 1 &&
		transfer === 13 &&
		matrix === 0, 'Codec sRGB RGBA8 profile');
	return Object.freeze({ width, height, model: 'srgb-rgba8', rgba: payload });
}
