// Protocol6 accounting is descriptive; Rust rederives it from the frozen source.
import { refusal } from './codec-worker-protocol.js';

function copyContext(bytes) {
	if (!(bytes instanceof Uint8Array) || bytes.length !== 64)
		throw refusal('InputSchema', 'Invalid catalog export context');
	const copy = new Uint8Array(64);
	for (let i = 0; i < 64; i++) copy[i] = bytes[i];
	return copy;
}
export function freezeCatalogSpatialContext(bytes, cap) {
	const copy = copyContext(bytes);
	const view = new DataView(copy.buffer);
	const n = (at) => view.getBigUint64(at);
	if (
		view.getUint16(0) !== 6 ||
		view.getUint16(2) !== 1 ||
		view.getUint32(4) > 2048 ||
		view.getUint32(8) > view.getUint32(4) ||
		copy.subarray(12, 16).some((b) => b !== 0) ||
		copy.subarray(48).some((b) => b !== 0) ||
		(!view.getUint32(8) && n(24) !== 0n) ||
		n(32) > 268435456n ||
		n(40) < n(24) ||
		n(40) > BigInt(cap)
	)
		throw refusal(
			'ResourceLimit',
			'Catalog region accounting exceeds its profile',
		);
	return Object.freeze(Array.from({ length: 64 }, (_, i) => copy[i]));
}
export function freezeCatalogContext(bytes, spatial, cap) {
	const copy = copyContext(bytes);
	const view = new DataView(copy.buffer);
	const n = (at) => view.getBigUint64(at);
	const region = new DataView(Uint8Array.from(spatial).buffer);
	const stored = view.getUint32(4),
		enabled = view.getUint32(8),
		unique = view.getUint32(12);
	if (
		view.getUint16(0) !== 6 ||
		view.getUint16(2) !== 1 ||
		stored < 1 ||
		stored > 2048 ||
		enabled > stored ||
		unique > enabled ||
		unique > 256 ||
		(enabled === 0) !== (unique === 0) ||
		copy.subarray(56).some((b) => b !== 0) ||
		(!enabled && (n(16) !== 0n || n(24) !== 0n || n(32) !== 0n)) ||
		(unique > 0 && n(16) === 0n) ||
		n(24) < BigInt(unique) * 256n ||
		n(32) < n(24) ||
		n(40) !== n(24) + region.getBigUint64(24) ||
		n(48) !== n(16) + region.getBigUint64(32) ||
		n(48) > 268435456n ||
		n(32) > 67108864n ||
		n(40) > 67108864n ||
		n(32) > BigInt(cap) ||
		n(40) > BigInt(cap)
	)
		throw refusal(
			'ResourceLimit',
			'Catalog preparation accounting exceeds its profile',
		);
	return Object.freeze(Array.from({ length: 64 }, (_, i) => copy[i]));
}
