// Export7 descriptions are rederived by Rust; no context grants bake authority.
import { refusal } from './codec-worker-protocol.js';
import { freezeCatalogSpatialContext } from './source-catalog-worker-context.js';
function copy(bytes, length) {
	if (!(bytes instanceof Uint8Array) || bytes.length !== length)
		throw refusal('InputSchema', 'Invalid prepared export context');
	const result = new Uint8Array(length);
	for (let i = 0; i < length; i++) result[i] = bytes[i];
	return result;
}
function frozen(bytes) {
	return Object.freeze(
		Array.from({ length: bytes.length }, (_, i) => bytes[i]),
	);
}
export function freezeProgramSpatialContext(bytes, cap) {
	const result = copy(bytes, 64);
	const view = new DataView(result.buffer);
	if (view.getUint16(2) !== 2)
		throw refusal('InputSchema', 'Unsupported prepared region contract');
	// Contract2 retains region6 geometry/accounting, with independent capability
	// resolution in program128. The old contract bytes are never modified.
	view.setUint16(2, 1);
	freezeCatalogSpatialContext(result, cap);
	view.setUint16(2, 2);
	return frozen(result);
}
export function freezeProgramContext(bytes, spatial, cap) {
	const result = copy(bytes, 128);
	const view = new DataView(result.buffer);
	if (!Array.isArray(spatial) || spatial.length !== 64)
		throw refusal('InputSchema', 'Invalid prepared region bytes');
	const regionBytes = new Uint8Array(64);
	for (let i = 0; i < 64; i++) {
		const value = spatial[i];
		if (!Number.isInteger(value) || value < 0 || value > 255)
			throw refusal('InputSchema', 'Invalid prepared region byte');
		regionBytes[i] = value;
	}
	const region = new DataView(regionBytes.buffer);
	const n = (at) => view.getBigUint64(at);
	const required = view.getUint32(4),
		stored = view.getUint32(8),
		enabled = view.getUint32(12),
		unique = view.getUint32(16);
	if (
		view.getUint16(0) !== 1 ||
		view.getUint16(2) !== 1 ||
		view.getUint32(20) !== 0 ||
		required > 256 ||
		stored > 2048 ||
		enabled > stored ||
		required > stored ||
		unique > enabled ||
		unique > 256 ||
		(unique === 0 && (n(24) !== 0n || n(32) !== 0n || n(40) !== 0n)) ||
		(unique > 0 && (n(24) === 0n || n(32) < BigInt(unique) * 256n)) ||
		n(40) < n(32) ||
		n(48) !== n(32) + region.getBigUint64(24) ||
		n(56) !== n(24) + region.getBigUint64(32)
	)
		throw refusal('InputSchema', 'Noncanonical prepared export accounting');
	if (
		n(40) > 67108864n ||
		n(48) > 67108864n ||
		n(56) > 268435456n ||
		n(40) > BigInt(cap) ||
		n(48) > BigInt(cap)
	)
		throw refusal(
			'ResourceLimit',
			'Prepared export accounting exceeds its profile',
		);
	// Enabled legacy direct filters may exist without any prepared program.
	return frozen(result);
}
